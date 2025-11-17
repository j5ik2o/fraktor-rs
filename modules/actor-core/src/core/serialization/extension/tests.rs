use alloc::{
  borrow::Cow,
  boxed::Box,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::any::{Any, TypeId};

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};
use hashbrown::HashMap;
use portable_atomic::{AtomicUsize, Ordering};

use super::*;
use crate::core::{
  actor_prim::{
    Pid,
    actor_ref::{ActorRefGeneric, NullSender},
  },
  dead_letter::DeadLetterReason,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  serialization::{
    builtin, call_scope::SerializationCallScope, error::SerializationError, error_event::SerializationErrorEvent,
    not_serializable_error::NotSerializableError, serialization_registry::SerializationRegistryGeneric,
    serialization_setup::SerializationSetup, serialized_message::SerializedMessage, serializer::Serializer,
    serializer_id::SerializerId, string_manifest_serializer::SerializerWithStringManifest,
    transport_information::TransportInformation,
  },
  system::ActorSystemGeneric,
};

impl<TB: RuntimeToolbox + 'static> SerializationExtensionGeneric<TB> {
  /// Returns the underlying registry handle (testing only).
  pub const fn registry(&self) -> &ArcShared<SerializationRegistryGeneric<TB>> {
    &self.registry
  }
}

#[derive(Debug, PartialEq)]
struct TestPayload(u8);

struct TestSerializer {
  id: SerializerId,
}

impl TestSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for TestSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(
    &self,
    value: &(dyn Any + Send + Sync),
  ) -> Result<Vec<u8>, crate::core::serialization::error::SerializationError> {
    let payload = value.downcast_ref::<TestPayload>().expect("TestPayload expected");
    Ok(vec![payload.0])
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send>, crate::core::serialization::error::SerializationError> {
    Ok(Box::new(TestPayload(bytes.first().copied().unwrap_or_default())))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

fn build_extension<TB: RuntimeToolbox + 'static>(
  manifest: Option<&str>,
) -> (SerializationExtensionGeneric<TB>, SerializerId) {
  let serializer_id = SerializerId::try_from(300).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TestSerializer::new(serializer_id));
  let mut builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("test", serializer_id, serializer)
    .expect("register")
    .set_fallback("test")
    .expect("fallback")
    .bind::<TestPayload>("test")
    .expect("bind");
  if let Some(manifest) = manifest {
    builder = builder.bind_remote_manifest::<TestPayload>(manifest).expect("manifest");
    builder = builder.require_manifest_for_scope(SerializationCallScope::Remote);
  }
  let setup: SerializationSetup = builder.build().expect("build");
  let system = ActorSystemGeneric::<TB>::new_empty();
  (SerializationExtensionGeneric::new(&system, setup), serializer_id)
}

fn serialize_and_deserialize(extension: &SerializationExtensionGeneric<NoStdToolbox>) -> TestPayload {
  let payload = TestPayload(42);
  let serialized = extension.serialize(&payload, SerializationCallScope::Local).expect("serialize");
  let any = extension.deserialize(&serialized, Some(TypeId::of::<TestPayload>())).expect("deserialize");
  *any.downcast::<TestPayload>().expect("downcast")
}

#[test]
fn serialize_local_round_trip() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let result = serialize_and_deserialize(&extension);
  assert_eq!(result, TestPayload(42));
}

#[test]
fn serialize_remote_attaches_manifest() {
  let (extension, _) = build_extension::<NoStdToolbox>(Some("example.Manifest"));
  let payload = TestPayload(7);
  let serialized = extension.serialize(&payload, SerializationCallScope::Remote).expect("serialize");
  assert_eq!(serialized.manifest(), Some("example.Manifest"));
}

#[test]
fn with_transport_information_sets_scope_temporarily() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  assert!(extension.current_transport_information().is_none());
  let info = TransportInformation::new(Some("fraktor://sys@host".into()));
  let value = extension.with_transport_information(info.clone(), || extension.current_transport_information());
  assert_eq!(value.as_ref(), Some(&info));
  assert!(extension.current_transport_information().is_none());
}

#[test]
fn serialized_actor_path_prefers_transport_address() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let actor_ref = ActorRefGeneric::<NoStdToolbox>::new(Pid::new(1, 0), ArcShared::new(NullSender));
  let info = TransportInformation::new(Some("fraktor://sys@host:2552".into()));
  let path = extension.with_transport_information(info, || extension.serialized_actor_path(&actor_ref)).expect("path");
  assert!(path.starts_with("fraktor://sys@host:2552"));
}

#[test]
fn shutdown_rejects_future_serialization() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  extension.shutdown();
  let error = extension.serialize(&TestPayload(1), SerializationCallScope::Local).expect_err("should fail");
  assert!(matches!(error, SerializationError::Uninitialized));
}

struct ManifestSerializer {
  id:                  SerializerId,
  from_manifest_calls: AtomicUsize,
}

impl ManifestSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id, from_manifest_calls: AtomicUsize::new(0) }
  }
}

impl Serializer for ManifestSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TestPayload>().expect("payload");
    Ok(vec![payload.0])
  }

  fn from_binary(&self, bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(TestPayload(bytes[0])))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for ManifestSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("manifest::TestPayload")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    self.from_manifest_calls.fetch_add(1, Ordering::Relaxed);
    Ok(Box::new(TestPayload(bytes[0])))
  }
}

fn build_manifest_extension() -> SerializationExtensionGeneric<NoStdToolbox> {
  let serializer_id = SerializerId::try_from(333).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(ManifestSerializer::new(serializer_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("manifest", serializer_id, serializer)
    .expect("register")
    .set_fallback("manifest")
    .expect("fallback")
    .bind::<TestPayload>("manifest")
    .expect("bind");
  let setup = builder.build().expect("build");
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  SerializationExtensionGeneric::new(&system, setup)
}

#[test]
fn string_manifest_serializer_supplies_manifest() {
  let extension = build_manifest_extension();
  let payload = TestPayload(5);
  let serialized = extension.serialize(&payload, SerializationCallScope::Remote).expect("serialize");
  assert_eq!(serialized.manifest(), Some("manifest::TestPayload"));
}

#[test]
fn deserialize_prefers_from_binary_with_manifest() {
  let extension = build_manifest_extension();
  let payload =
    SerializedMessage::new(SerializerId::try_from(333).unwrap(), Some("manifest::TestPayload".into()), vec![9]);
  let any = extension.deserialize(&payload, None).expect("deserialize");
  assert_eq!(*any.downcast::<TestPayload>().unwrap(), TestPayload(9));
}

#[test]
fn not_serializable_publishes_event_and_deadletter() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let serialization_events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl =
    ArcShared::new(SerializationEventWatcher::new(serialization_events.clone(), log_messages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let serializer_id = SerializerId::try_from(401).expect("id");
  let fallback: ArcShared<dyn Serializer> = ArcShared::new(FailingSerializer::new(serializer_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("failing", serializer_id, fallback)
    .expect("register")
    .set_fallback("failing")
    .expect("fallback");
  let setup = builder.build().expect("build");
  let extension = SerializationExtensionGeneric::new(&system, setup);

  let error = extension.serialize(&TestPayload(1), SerializationCallScope::Local).expect_err("should fail");
  assert!(matches!(error, SerializationError::NotSerializable(_)));

  let events = serialization_events.lock();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].type_name(), core::any::type_name::<TestPayload>());

  let dead_letters = system.dead_letters();
  assert!(dead_letters.iter().any(|entry| entry.reason() == DeadLetterReason::SerializationError));

  assert!(!log_messages.lock().is_empty());
}

#[test]
fn not_serializable_event_records_pid_and_transport() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let serialization_events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl =
    ArcShared::new(SerializationEventWatcher::new(serialization_events.clone(), log_messages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let serializer_id = SerializerId::try_from(402).expect("id");
  let failing: ArcShared<dyn Serializer> = ArcShared::new(FailingSerializer::new(serializer_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("failing", serializer_id, failing)
    .expect("register")
    .set_fallback("failing")
    .expect("fallback");
  let setup = builder.build().expect("build");
  let extension = SerializationExtensionGeneric::new(&system, setup);

  let pid = Pid::new(77, 1);
  let info = TransportInformation::new(Some("fraktor://sys@host:2552".into()));
  let error = extension
    .with_transport_information(info.clone(), || {
      extension.serialize_for(&TestPayload(1), SerializationCallScope::Remote, Some(pid))
    })
    .expect_err("should fail");
  assert!(matches!(error, SerializationError::NotSerializable(_)));

  let events = serialization_events.lock();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].pid(), Some(pid));
  assert_eq!(events[0].transport_hint(), Some("fraktor://sys@host:2552"));

  let dead_letters = system.dead_letters();
  assert!(
    dead_letters
      .iter()
      .any(|entry| { entry.reason() == DeadLetterReason::SerializationError && entry.recipient() == Some(pid) })
  );

  let log_entries = log_messages.lock();
  assert!(!log_entries.is_empty());
}

#[test]
fn manifest_route_falls_back_to_legacy_serializer() {
  let (current_id, legacy_id) =
    (SerializerId::try_from(420).expect("current"), SerializerId::try_from(421).expect("legacy"));
  let current: ArcShared<dyn Serializer> = ArcShared::new(VersionedSerializer::new(current_id));
  let legacy: ArcShared<dyn Serializer> = ArcShared::new(LegacySerializer::new(legacy_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("current", current_id, current)
    .expect("register current")
    .register_serializer("legacy", legacy_id, legacy)
    .expect("register legacy")
    .set_fallback("current")
    .expect("fallback")
    .bind::<TestPayload>("current")
    .expect("bind")
    .register_manifest_route("legacy.Manifest", 1, "legacy")
    .expect("route");
  let setup = builder.build().expect("build");
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let extension = SerializationExtensionGeneric::new(&system, setup);

  let message = SerializedMessage::new(current_id, Some("legacy.Manifest".into()), vec![11]);
  let any = extension.deserialize(&message, Some(TypeId::of::<TestPayload>())).expect("deserialize");
  assert_eq!(*any.downcast::<TestPayload>().unwrap(), TestPayload(11));
}

#[test]
fn manifest_route_failure_surfaces_not_serializable_with_manifest() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let serialization_events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl =
    ArcShared::new(SerializationEventWatcher::new(serialization_events.clone(), log_messages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let serializer_id = SerializerId::try_from(422).expect("serializer");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(VersionedSerializer::new(serializer_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("current", serializer_id, serializer)
    .expect("register")
    .set_fallback("current")
    .expect("fallback")
    .bind::<TestPayload>("current")
    .expect("bind");
  let setup = builder.build().expect("build");
  let extension = SerializationExtensionGeneric::new(&system, setup);

  let serialized = SerializedMessage::new(serializer_id, Some("missing.Manifest".into()), vec![99]);
  let error = extension.deserialize(&serialized, Some(TypeId::of::<TestPayload>())).expect_err("should fail");
  match error {
    | SerializationError::NotSerializable(payload) => {
      assert_eq!(payload.manifest(), Some("missing.Manifest"));
      assert_eq!(payload.serializer_id(), Some(serializer_id));
    },
    | other => panic!("unexpected error: {other:?}"),
  }

  let events = serialization_events.lock();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].manifest(), Some("missing.Manifest"));
  assert!(log_messages.lock().iter().any(|entry| entry.contains("manifest 'missing.Manifest' not resolved")));
}

#[derive(Debug, PartialEq)]
struct SecondaryPayload(u8);

#[test]
fn runtime_binding_without_manifest_in_remote_scope_fails() {
  let serializer_id = SerializerId::try_from(430).expect("serializer");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TestSerializer::new(serializer_id));
  let secondary_id = SerializerId::try_from(431).expect("secondary");
  let secondary: ArcShared<dyn Serializer> = ArcShared::new(SecondarySerializer::new(secondary_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("main", serializer_id, serializer)
    .expect("register")
    .register_serializer("secondary", secondary_id, secondary)
    .expect("register secondary")
    .set_fallback("main")
    .expect("fallback")
    .bind::<TestPayload>("main")
    .expect("bind")
    .bind_remote_manifest::<TestPayload>("test.Manifest")
    .expect("manifest")
    .require_manifest_for_scope(SerializationCallScope::Remote);
  let setup = builder.build().expect("build");
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let extension = SerializationExtensionGeneric::new(&system, setup);

  extension
    .register_binding(TypeId::of::<SecondaryPayload>(), core::any::type_name::<SecondaryPayload>(), secondary_id)
    .expect("dynamic binding");
  let error =
    extension.serialize_for(&SecondaryPayload(1), SerializationCallScope::Remote, None).expect_err("manifest missing");
  assert!(matches!(error, SerializationError::ManifestMissing { scope: SerializationCallScope::Remote }));
}

#[test]
fn shutdown_blocks_deserialize_and_actor_path_calls() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let payload = TestPayload(5);
  let serialized = extension.serialize(&payload, SerializationCallScope::Local).expect("serialize");
  extension.shutdown();
  let error = extension.deserialize(&serialized, Some(TypeId::of::<TestPayload>())).expect_err("should fail");
  assert!(matches!(error, SerializationError::Uninitialized));

  let actor_ref = ActorRefGeneric::<NoStdToolbox>::new(Pid::new(2, 0), ArcShared::new(NullSender));
  let path_error = extension.serialized_actor_path(&actor_ref).expect_err("should fail");
  assert!(matches!(path_error, SerializationError::Uninitialized));
}

#[test]
fn cache_resolution_emits_hit_and_binding_logs() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let serialization_events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(SerializationEventWatcher::new(serialization_events, log_messages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let serializer_id = SerializerId::try_from(512).expect("serializer");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(TestSerializer::new(serializer_id));
  let builder = crate::core::serialization::builder::SerializationSetupBuilder::new()
    .register_serializer("test", serializer_id, serializer)
    .expect("register")
    .set_fallback("test")
    .expect("fallback")
    .bind::<TestPayload>("test")
    .expect("bind");
  let setup = builder.build().expect("build");
  let extension = SerializationExtensionGeneric::new(&system, setup);

  log_messages.lock().clear();
  extension.serialize(&TestPayload(21), SerializationCallScope::Local).expect("serialize miss");
  extension.serialize(&TestPayload(22), SerializationCallScope::Local).expect("serialize hit");

  let logs = log_messages.lock();
  assert!(logs.iter().any(|entry| entry.contains("binding resolved")));
  assert!(logs.iter().any(|entry| entry.contains("cache hit")));
}

#[test]
fn builtin_serializer_collision_emits_warning() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let serialization_events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let log_messages = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(SerializationEventWatcher::new(serialization_events, log_messages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let serializer_id = SerializerId::from_raw(1);
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(builtin::NullSerializer::new(serializer_id));
  let mut serializers = HashMap::new();
  serializers.insert(serializer_id, serializer);
  let setup = SerializationSetup::testing_from_raw(
    serializers,
    HashMap::new(),
    HashMap::new(),
    HashMap::new(),
    HashMap::new(),
    Vec::new(),
    serializer_id,
    Vec::new(),
  );
  let _extension = SerializationExtensionGeneric::new(&system, setup);

  let logs = log_messages.lock();
  assert!(logs.iter().any(|entry| entry.contains("collision")));
}

struct SerializationEventWatcher {
  serialization_events: ArcShared<NoStdMutex<Vec<SerializationErrorEvent>>>,
  log_messages:         ArcShared<NoStdMutex<Vec<String>>>,
}

impl SerializationEventWatcher {
  fn new(
    serialization_events: ArcShared<NoStdMutex<Vec<SerializationErrorEvent>>>,
    log_messages: ArcShared<NoStdMutex<Vec<String>>>,
  ) -> Self {
    Self { serialization_events, log_messages }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for SerializationEventWatcher {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    match event {
      | EventStreamEvent::Serialization(payload) => {
        self.serialization_events.lock().push(payload.clone());
      },
      | EventStreamEvent::Log(entry) => {
        self.log_messages.lock().push(entry.message().to_string());
      },
      | _ => {},
    }
  }
}

struct FailingSerializer {
  id: SerializerId,
}

impl FailingSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for FailingSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Err(SerializationError::NotSerializable(NotSerializableError::new(
      core::any::type_name::<TestPayload>(),
      Some(self.id),
      None,
      None,
      None,
    )))
  }

  fn from_binary(&self, _bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
    Err(SerializationError::InvalidFormat)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

struct VersionedSerializer {
  id: SerializerId,
}

impl VersionedSerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for VersionedSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TestPayload>().expect("payload");
    Ok(vec![payload.0])
  }

  fn from_binary(&self, bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(TestPayload(bytes[0])))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for VersionedSerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("current.Manifest")
  }

  fn from_binary_with_manifest(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError> {
    if manifest == "current.Manifest" {
      return Ok(Box::new(TestPayload(bytes[0])));
    }
    Err(SerializationError::UnknownManifest(manifest.to_string()))
  }
}

struct LegacySerializer {
  id: SerializerId,
}

impl LegacySerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for LegacySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<TestPayload>().expect("payload");
    Ok(vec![payload.0])
  }

  fn from_binary(&self, bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(TestPayload(bytes[0])))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }

  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    Some(self)
  }
}

impl SerializerWithStringManifest for LegacySerializer {
  fn manifest(&self, _message: &(dyn Any + Send + Sync)) -> Cow<'_, str> {
    Cow::Borrowed("legacy.Manifest")
  }

  fn from_binary_with_manifest(
    &self,
    bytes: &[u8],
    _manifest: &str,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(TestPayload(bytes[0])))
  }
}

struct SecondarySerializer {
  id: SerializerId,
}

impl SecondarySerializer {
  fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for SecondarySerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    false
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<SecondaryPayload>().expect("secondary payload");
    Ok(vec![payload.0])
  }

  fn from_binary(&self, bytes: &[u8], _type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(SecondaryPayload(bytes[0])))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

#[test]
fn builtin_serializers_support_primitives() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);

  let bool_msg = extension.serialize(&true, SerializationCallScope::Local).expect("bool encode");
  let bool_value = extension.deserialize(&bool_msg, Some(TypeId::of::<bool>())).expect("bool decode");
  assert!(*bool_value.downcast::<bool>().unwrap());

  let number: i32 = 12345;
  let int_msg = extension.serialize(&number, SerializationCallScope::Local).expect("i32 encode");
  let int_value = extension.deserialize(&int_msg, Some(TypeId::of::<i32>())).expect("i32 decode");
  assert_eq!(*int_value.downcast::<i32>().unwrap(), number);

  let text = String::from("hello");
  let text_msg = extension.serialize(&text, SerializationCallScope::Local).expect("string encode");
  let text_value = extension.deserialize(&text_msg, Some(TypeId::of::<String>())).expect("string decode");
  assert_eq!(*text_value.downcast::<String>().unwrap(), text);

  let bytes = vec![1_u8, 2, 3];
  let bytes_msg = extension.serialize(&bytes, SerializationCallScope::Local).expect("bytes encode");
  let bytes_value = extension.deserialize(&bytes_msg, Some(TypeId::of::<Vec<u8>>())).expect("bytes decode");
  assert_eq!(*bytes_value.downcast::<Vec<u8>>().unwrap(), bytes);
}

#[test]
fn actor_ref_serialization_uses_helper() {
  let (extension, _) = build_extension::<NoStdToolbox>(None);
  let actor_ref = ActorRefGeneric::<NoStdToolbox>::new(Pid::new(99, 0), ArcShared::new(NullSender));
  let info = TransportInformation::new(Some("fraktor://sys@host:2552".into()));
  let message = extension
    .with_transport_information(info, || extension.serialize(&actor_ref, SerializationCallScope::Remote))
    .expect("serialize");
  let decoded = extension.deserialize(&message, Some(TypeId::of::<String>())).expect("decode");
  let path = decoded.downcast::<String>().unwrap();
  assert!(path.starts_with("fraktor://sys@host:2552"));
}
