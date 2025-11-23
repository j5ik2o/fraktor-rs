//! Serialization extension wiring for actor systems.

#[cfg(test)]
mod tests;

use alloc::{
  borrow::ToOwned,
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  any::{Any, TypeId, type_name_of_val},
  marker::PhantomData,
  sync::atomic::{AtomicBool, Ordering},
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  actor_prim::{Pid, actor_ref::ActorRefGeneric},
  dead_letter::DeadLetterReason,
  event_stream::EventStreamEvent,
  extension::Extension,
  logging::LogLevel,
  messaging::AnyMessageGeneric,
  serialization::{
    builtin,
    call_scope::SerializationCallScope,
    error::SerializationError,
    error_event::SerializationErrorEvent,
    not_serializable_error::NotSerializableError,
    serialization_registry::{SerializationRegistryGeneric, SerializerResolutionOrigin},
    serialization_setup::SerializationSetup,
    serialized_message::SerializedMessage,
    serializer_id::SerializerId,
    transport_information::TransportInformation,
  },
  system::{ActorSystemGeneric, SystemStateGeneric},
};

/// Serialization extension type alias for the default toolbox.
pub type SerializationExtension = SerializationExtensionGeneric<NoStdToolbox>;

/// Serialization extension registered within the actor system.
pub struct SerializationExtensionGeneric<TB: RuntimeToolbox + 'static> {
  registry:        ArcShared<SerializationRegistryGeneric<TB>>,
  setup:           SerializationSetup,
  system_state:    ArcShared<SystemStateGeneric<TB>>,
  transport_stack: ToolboxMutex<Vec<TransportInformation>, TB>,
  uninitialized:   AtomicBool,
  _marker:         PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> SerializationExtensionGeneric<TB> {
  /// Creates the extension from the provided setup.
  ///
  /// # Panics
  ///
  /// Panics if the built-in serializers fail to register. This should not happen
  /// under normal conditions and indicates a serious configuration issue.
  #[must_use]
  pub fn new(system: &ActorSystemGeneric<TB>, setup: SerializationSetup) -> Self {
    let registry = ArcShared::new(SerializationRegistryGeneric::from_setup(&setup));
    let state = system.state();
    {
      // builtinシリアライザの登録を試み、失敗時には警告ログを出力してから継続
      // 通常は発生しないが、システム構成に重大な問題がある場合にパニックする
      if let Err(error) = builtin::register_defaults(&registry, |name, id| {
        let message = format!("serializer collision detected for built-in {name} (id {:?})", id);
        state.emit_log(LogLevel::Warn, message, None);
      }) {
        let message = format!("critical: failed to register builtin serializers: {error:?}");
        state.emit_log(LogLevel::Error, message, None);
        panic!("failed to register builtin serializers: {error:?}");
      }
    }
    Self {
      registry,
      setup,
      system_state: state,
      transport_stack: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      uninitialized: AtomicBool::new(false),
      _marker: PhantomData,
    }
  }

  /// Serializes the provided object respecting the specified scope.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError` if:
  /// - The extension has been shut down (`Uninitialized`)
  /// - No suitable serializer can be found for the object's type (`NotSerializable`)
  /// - Serialization fails due to data conversion issues
  /// - Manifest generation fails when required for the specified scope
  pub fn serialize(
    &self,
    obj: &(dyn Any + Send + Sync),
    scope: SerializationCallScope,
  ) -> Result<SerializedMessage, SerializationError> {
    self.serialize_for(obj, scope, None)
  }

  /// Serializes the object while annotating the originating pid for diagnostics.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError` if:
  /// - The extension has been shut down (`Uninitialized`)
  /// - No suitable serializer can be found for the object's type (`NotSerializable`)
  /// - Serialization fails due to data conversion issues
  /// - Manifest generation fails when required for the specified scope
  pub fn serialize_for(
    &self,
    obj: &(dyn Any + Send + Sync),
    scope: SerializationCallScope,
    pid: Option<Pid>,
  ) -> Result<SerializedMessage, SerializationError> {
    self.serialize_internal(obj, scope, pid)
  }

  /// Deserializes the message using the registered serializers.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError` if:
  /// - The extension has been shut down (`Uninitialized`)
  /// - The specified serializer ID is not registered
  /// - Deserialization fails due to data format issues
  /// - Manifest routing fails to resolve the message type
  pub fn deserialize(
    &self,
    msg: &SerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    self.ensure_active()?;
    let transport_hint = self.current_transport_information();
    let serializer = match self.registry.serializer_by_id(msg.serializer_id()) {
      | Ok(serializer) => serializer,
      | Err(error) => return Err(self.handle_error(error, None, transport_hint)),
    };
    let result = if let Some(manifest) = msg.manifest()
      && let Some(provider) = serializer.as_string_manifest()
    {
      provider.from_binary_with_manifest(msg.bytes(), manifest)
    } else {
      serializer.from_binary(msg.bytes(), type_hint)
    };
    match result {
      | Ok(value) => Ok(value),
      | Err(SerializationError::UnknownManifest(manifest)) => {
        self.deserialize_with_manifest_routes(manifest, msg, type_hint, transport_hint)
      },
      | Err(error) => Err(self.handle_error(error, None, transport_hint)),
    }
  }

  /// Executes the closure with the specified transport information installed.
  pub fn with_transport_information<R>(&self, info: TransportInformation, f: impl FnOnce() -> R) -> R {
    let guard = TransportScopeGuard::<TB>::push(&self.transport_stack, info);
    let result = f();
    drop(guard);
    result
  }

  /// Returns the current transport information (if any).
  #[must_use]
  pub fn current_transport_information(&self) -> Option<TransportInformation> {
    self.transport_stack.lock().last().cloned()
  }

  /// Converts the actor reference to a serialized actor path string.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError::Uninitialized` if the extension has been shut down.
  pub fn serialized_actor_path(&self, actor_ref: &ActorRefGeneric<TB>) -> Result<String, SerializationError> {
    self.ensure_active()?;
    if self.current_transport_information().is_none() && self.system_state.has_partial_canonical_authority() {
      let payload = NotSerializableError::new("ActorRef", None, None, Some(actor_ref.pid()), None);
      self.publish_not_serializable(&payload);
      return Err(SerializationError::NotSerializable(payload));
    }
    if let Some(info) = self.current_transport_information()
      && let Some(address) = info.address()
    {
      let mut normalized = address.to_string();
      let path = Self::actor_path(actor_ref);
      Self::append_path(&mut normalized, &path);
      return Ok(normalized);
    }
    if let Some(canonical) = actor_ref.canonical_path()
      && canonical.parts().authority_endpoint().is_some()
    {
      return Ok(canonical.to_canonical_uri());
    }
    let path = Self::actor_path(actor_ref);
    Ok(format!("local://{path}"))
  }

  /// Registers an additional binding at runtime.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError` if:
  /// - A binding for the given type ID already exists with a different serializer
  /// - The serializer ID is not registered in the system
  pub fn register_binding(
    &self,
    type_id: TypeId,
    type_name: impl Into<String>,
    serializer_id: SerializerId,
  ) -> Result<(), SerializationError> {
    self.registry.register_binding(type_id, type_name, serializer_id)
  }

  fn append_path(prefix: &mut String, path: &str) {
    if !prefix.ends_with('/') && !path.starts_with('/') {
      prefix.push('/');
    }
    if path.starts_with('/') && prefix.ends_with('/') {
      prefix.push_str(path.trim_start_matches('/'));
    } else {
      prefix.push_str(path);
    }
  }

  /// Shuts down the extension making further calls fail.
  pub fn shutdown(&self) {
    if self.uninitialized.swap(true, Ordering::SeqCst) {
      return;
    }
    self.transport_stack.lock().clear();
    self.registry.clear_cache();
  }

  fn ensure_active(&self) -> Result<(), SerializationError> {
    if self.uninitialized.load(Ordering::SeqCst) {
      return Err(SerializationError::Uninitialized);
    }
    Ok(())
  }

  const fn resolve_scope(
    requested: SerializationCallScope,
    transport: Option<&TransportInformation>,
  ) -> SerializationCallScope {
    match (requested, transport) {
      | (SerializationCallScope::Local, Some(_)) => SerializationCallScope::Remote,
      | (scope, _) => scope,
    }
  }

  fn manifest_for(
    &self,
    type_id: TypeId,
    scope: SerializationCallScope,
    fallback: Option<String>,
  ) -> Result<Option<String>, SerializationError> {
    if let Some(builder_manifest) = self.setup.manifest_for(type_id) {
      return Ok(Some(builder_manifest.to_owned()));
    }
    let required = self.setup.manifest_required_scopes().contains(&scope);
    if required {
      return fallback.map(Some).ok_or(SerializationError::ManifestMissing { scope });
    }
    Ok(fallback)
  }

  fn actor_path(actor_ref: &ActorRefGeneric<TB>) -> String {
    if let Some(path) = actor_ref.path() {
      return path.to_string();
    }
    fallback_path(actor_ref.pid())
  }

  fn deserialize_with_manifest_routes(
    &self,
    manifest: String,
    msg: &SerializedMessage,
    type_hint: Option<TypeId>,
    transport_hint: Option<TransportInformation>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let candidates = self.registry.serializers_for_manifest(&manifest);
    for serializer in candidates {
      let outcome = if let Some(provider) = serializer.as_string_manifest() {
        provider.from_binary_with_manifest(msg.bytes(), &manifest)
      } else {
        serializer.from_binary(msg.bytes(), type_hint)
      };
      match outcome {
        | Ok(value) => {
          let message = format!("manifest '{manifest}' resolved via serializer {:?}", serializer.identifier());
          self.system_state.emit_log(LogLevel::Info, message, None);
          return Ok(value);
        },
        | Err(SerializationError::UnknownManifest(_)) => continue,
        | Err(error) => return Err(self.handle_error(error, None, transport_hint)),
      }
    }
    self.fail_manifest_route(manifest, msg, transport_hint)
  }

  fn fail_manifest_route(
    &self,
    manifest: String,
    msg: &SerializedMessage,
    transport_hint: Option<TransportInformation>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let log_message = format!("manifest '{manifest}' not resolved (serializer {:?})", msg.serializer_id());
    self.system_state.emit_log(LogLevel::Warn, log_message, None);
    let payload =
      NotSerializableError::new(manifest.clone(), Some(msg.serializer_id()), Some(manifest), None, transport_hint);
    Err(self.handle_error(SerializationError::NotSerializable(payload), None, None))
  }

  fn log_resolution(&self, type_name: &str, serializer_id: SerializerId, origin: SerializerResolutionOrigin) {
    let (level, source) = match origin {
      | SerializerResolutionOrigin::Cache => (LogLevel::Debug, "serialization cache hit"),
      | SerializerResolutionOrigin::Binding => (LogLevel::Info, "serialization binding resolved"),
      | SerializerResolutionOrigin::Fallback => (LogLevel::Info, "serialization fallback resolved"),
    };
    let message = format!("{source} for type {type_name} -> {:?}", serializer_id);
    self.system_state.emit_log(level, message, None);
  }

  fn serialize_internal(
    &self,
    obj: &(dyn Any + Send + Sync),
    scope: SerializationCallScope,
    pid: Option<Pid>,
  ) -> Result<SerializedMessage, SerializationError> {
    self.ensure_active()?;
    if let Some(actor_ref) = obj.downcast_ref::<ActorRefGeneric<TB>>() {
      let path = self.serialized_actor_path(actor_ref)?;
      return self.serialize_internal(&path, scope, pid);
    }
    let transport_hint = self.current_transport_information();
    let effective_scope = Self::resolve_scope(scope, transport_hint.as_ref());
    let type_id = obj.type_id();
    let type_name = type_name_of_val(obj);
    let (serializer, origin) = match self.registry.serializer_for_type(type_id, type_name, transport_hint.clone()) {
      | Ok(value) => value,
      | Err(error) => return Err(self.handle_error(error, pid, transport_hint)),
    };
    self.log_resolution(type_name, serializer.identifier(), origin);
    let bytes = match serializer.to_binary(obj) {
      | Ok(bytes) => bytes,
      | Err(error) => return Err(self.handle_error(error, pid, transport_hint)),
    };
    let manifest_from_serializer = serializer.as_string_manifest().map(|provider| provider.manifest(obj).into_owned());
    let manifest = match self.manifest_for(type_id, effective_scope, manifest_from_serializer) {
      | Ok(manifest) => manifest,
      | Err(error) => return Err(self.handle_error(error, pid, transport_hint)),
    };
    Ok(SerializedMessage::new(serializer.identifier(), manifest, bytes))
  }

  fn handle_error(
    &self,
    error: SerializationError,
    pid: Option<Pid>,
    transport_hint: Option<TransportInformation>,
  ) -> SerializationError {
    match error {
      | SerializationError::NotSerializable(payload) => {
        let payload = payload.with_pid(pid).with_transport_hint(transport_hint);
        self.publish_not_serializable(&payload);
        SerializationError::NotSerializable(payload)
      },
      | other => other,
    }
  }

  fn publish_not_serializable(&self, payload: &NotSerializableError) {
    let event = SerializationErrorEvent::from_error(payload);
    let event_stream = self.system_state.event_stream();
    event_stream.publish(&EventStreamEvent::Serialization(event));

    let message: AnyMessageGeneric<TB> = AnyMessageGeneric::new(payload.clone());
    self.system_state.record_dead_letter(message, DeadLetterReason::SerializationError, payload.pid());

    let log_message = format!(
      "serialization failure for type {} (serializer: {:?}, manifest: {:?})",
      payload.type_name(),
      payload.serializer_id(),
      payload.manifest(),
    );
    self.system_state.emit_log(LogLevel::Warn, log_message, payload.pid());
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for SerializationExtensionGeneric<TB> {}

fn fallback_path(pid: Pid) -> String {
  format!("/pid/{}:{}", pid.value(), pid.generation())
}

struct TransportScopeGuard<'a, TB: RuntimeToolbox> {
  stack: &'a ToolboxMutex<Vec<TransportInformation>, TB>,
}

impl<'a, TB: RuntimeToolbox> TransportScopeGuard<'a, TB> {
  fn push(stack: &'a ToolboxMutex<Vec<TransportInformation>, TB>, info: TransportInformation) -> Self {
    stack.lock().push(info);
    Self { stack }
  }
}

impl<'a, TB: RuntimeToolbox> Drop for TransportScopeGuard<'a, TB> {
  fn drop(&mut self) {
    let _ = self.stack.lock().pop();
  }
}
