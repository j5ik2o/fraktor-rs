//! Serialization extension wiring for actor systems.

#[cfg(test)]
mod tests;

use alloc::{
  boxed::Box,
  format,
  string::{String, ToString},
  vec::Vec,
};
use core::{
  any::{type_name_of_val, Any, TypeId},
  marker::PhantomData,
  sync::atomic::{AtomicBool, Ordering},
};

use cellactor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{sync_mutex_like::SyncMutexLike, ArcShared, NoStdToolbox},
};

use crate::{
  Extension, ExtensionId, RuntimeToolbox, ToolboxMutex,
  actor_prim::{Pid, actor_ref::ActorRefGeneric},
  serialization::{
    call_scope::SerializationCallScope,
    error::SerializationError,
    serialized_message::SerializedMessage,
    serialization_registry::SerializationRegistryGeneric,
    serialization_setup::SerializationSetup,
    serializer_id::SerializerId,
    transport_information::TransportInformation,
  },
  system::ActorSystemGeneric,
};

/// Serialization extension type alias for the default toolbox.
pub type SerializationExtension = SerializationExtensionGeneric<NoStdToolbox>;

/// Serialization extension registered within the actor system.
pub struct SerializationExtensionGeneric<TB: RuntimeToolbox + 'static> {
  registry:        ArcShared<SerializationRegistryGeneric<TB>>,
  setup:           SerializationSetup,
  transport_stack: ToolboxMutex<Vec<TransportInformation>, TB>,
  uninitialized:   AtomicBool,
  _marker:         PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> SerializationExtensionGeneric<TB> {
  /// Creates the extension from the provided setup.
  #[must_use]
  pub fn new(_system: &ActorSystemGeneric<TB>, setup: SerializationSetup) -> Self {
    let registry = ArcShared::new(SerializationRegistryGeneric::from_setup(&setup));
    Self {
      registry,
      setup,
      transport_stack: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      uninitialized: AtomicBool::new(false),
      _marker: PhantomData,
    }
  }

  /// Serializes the provided object respecting the specified scope.
  pub fn serialize(&self, obj: &(dyn Any + Send + Sync), scope: SerializationCallScope) -> Result<SerializedMessage, SerializationError> {
    self.ensure_active()?;
    let transport_hint = self.current_transport_information();
    let effective_scope = self.resolve_scope(scope, transport_hint.as_ref());
    let type_id = obj.type_id();
    let type_name = type_name_of_val(obj);
    let serializer = self.registry.serializer_for_type(type_id, type_name, transport_hint.clone())?;
    let bytes = serializer.to_binary(obj)?;
    let manifest = self.manifest_for(type_id, effective_scope)?;
    Ok(SerializedMessage::new(serializer.identifier(), manifest, bytes))
  }

  /// Deserializes the message using the registered serializers.
  pub fn deserialize(
    &self,
    msg: &SerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    self.ensure_active()?;
    let serializer = self.registry.serializer_by_id(msg.serializer_id())?;
    serializer.from_binary(msg.bytes(), type_hint)
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
  pub fn serialized_actor_path(&self, actor_ref: &ActorRefGeneric<TB>) -> Result<String, SerializationError> {
    self.ensure_active()?;
    let path = self.actor_path(actor_ref);
    if let Some(info) = self.current_transport_information()
      && let Some(address) = info.address()
    {
      let mut normalized = address.to_string();
      if !normalized.ends_with('/') && !path.starts_with('/') {
        normalized.push('/');
      }
      if path.starts_with('/') && normalized.ends_with('/') {
        normalized.push_str(path.trim_start_matches('/'));
      } else {
        normalized.push_str(&path);
      }
      return Ok(normalized);
    }
    Ok(format!("local://{path}"))
  }

  /// Registers an additional binding at runtime.
  pub fn register_binding(
    &self,
    type_id: TypeId,
    type_name: impl Into<String>,
    serializer_id: SerializerId,
  ) -> Result<(), SerializationError> {
    self.registry.register_binding(type_id, type_name, serializer_id)
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

  fn resolve_scope(
    &self,
    requested: SerializationCallScope,
    transport: Option<&TransportInformation>,
  ) -> SerializationCallScope {
    match (requested, transport) {
      (SerializationCallScope::Local, Some(_)) => SerializationCallScope::Remote,
      (scope, _) => scope,
    }
  }

  fn manifest_for(
    &self,
    type_id: TypeId,
    scope: SerializationCallScope,
  ) -> Result<Option<String>, SerializationError> {
    let manifest = self.setup.manifest_for(type_id).map(String::from);
    let required = self.setup.manifest_required_scopes().iter().any(|candidate| *candidate == scope);
    if required && manifest.is_none() {
      return Err(SerializationError::ManifestMissing { scope });
    }
    Ok(manifest)
  }

  fn actor_path(&self, actor_ref: &ActorRefGeneric<TB>) -> String {
    if let Some(path) = actor_ref.path() {
      return path.to_string();
    }
    fallback_path(actor_ref.pid())
  }

  /// Returns the underlying registry handle (testing only).
  #[cfg(test)]
  pub fn registry(&self) -> &ArcShared<SerializationRegistryGeneric<TB>> {
    &self.registry
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for SerializationExtensionGeneric<TB> {}

/// Identifier used to register the serialization extension.
#[derive(Clone)]
pub struct SerializationExtensionId {
  setup: SerializationSetup,
}

impl SerializationExtensionId {
  /// Creates a new identifier for the provided setup.
  #[must_use]
  pub fn new(setup: SerializationSetup) -> Self {
    Self { setup }
  }
}

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for SerializationExtensionId {
  type Ext = SerializationExtensionGeneric<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
    SerializationExtensionGeneric::new(system, self.setup.clone())
  }
}

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
