#![cfg(any(test, feature = "test-support"))]

use alloc::string::String;
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, ActorContextGeneric, Pid,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationSetup, SerializationSetupBuilder, Serializer,
    SerializerId, StringSerializer,
  },
  system::{ActorSystemConfig, ActorSystemGeneric, RemoteWatchHook},
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  endpoint_writer::EndpointWriter, remote_actor_ref_provider::RemoteActorRefProvider,
  remoting_control::RemotingControl, remoting_control_handle::RemotingControlHandle,
  remoting_extension_config::RemotingExtensionConfig,
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("provider-tests");
  let system_config = ActorSystemConfig::default().with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystemGeneric::new_with_config(&props, &system_config).expect("system builds")
}

fn serialization_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(82).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register serializer")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("provider.String")
    .expect("manifest binding")
    .set_fallback("string")
    .expect("fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}

fn serialization_extension(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> ArcShared<SerializationExtensionGeneric<NoStdToolbox>> {
  ArcShared::new(SerializationExtensionGeneric::new(system, serialization_setup()))
}

fn provider(system: &ActorSystemGeneric<NoStdToolbox>) -> RemoteActorRefProvider {
  let serialization = serialization_extension(system);
  let writer = ArcShared::new(EndpointWriter::new(system.clone(), serialization));
  let control = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  control.start().expect("control start");
  let authority_manager = system.state().remote_authority_manager().clone();
  RemoteActorRefProvider::from_components(system.clone(), writer, control, authority_manager).expect("provider builds")
}

fn remote_path() -> ActorPath {
  let mut parts = ActorPathParts::with_authority("remote-app", Some(("127.0.0.1", 4100)));
  parts = parts.with_guardian(GuardianKind::User);
  let mut path = ActorPath::from_parts(parts);
  path = path.child("user");
  path = path.child("svc");
  path
}

#[test]
fn actor_ref_sends_messages_via_endpoint_writer() {
  let system = build_system();
  let provider = provider(&system);
  let writer = provider.writer_for_test();
  let remote = provider.actor_ref(remote_path()).expect("actor ref");

  remote.tell(AnyMessageGeneric::new("hello".to_string())).expect("send succeeds");

  let envelope = writer.try_next().expect("poll writer").expect("envelope exists");
  assert_eq!(envelope.recipient().to_relative_string(), "/user/user/svc");
  assert_eq!(envelope.remote_node().system(), "remote-app");
  assert_eq!(envelope.remote_node().host(), "127.0.0.1");
  assert_eq!(envelope.remote_node().port(), Some(4100));
}

#[test]
fn watch_remote_associates_authority() {
  let system = build_system();
  let provider = provider(&system);
  let mut parts = ActorPathParts::with_authority("remote-app", Some(("10.0.0.1", 9000)));
  parts = parts.with_guardian(GuardianKind::User);
  provider.watch_remote(parts).expect("watch succeeds");
}

#[test]
fn registers_remote_entry_for_remote_pid() {
  let system = build_system();
  let provider = provider(&system);
  let remote = provider.actor_ref(remote_path()).expect("actor ref");
  let registered = provider.registered_remote_pids_for_test();
  assert!(registered.contains(&remote.pid()));
}

#[test]
fn remote_watch_hook_tracks_watcher_lifecycle() {
  let system = build_system();
  let mut provider = provider(&system);
  let remote = provider.actor_ref(remote_path()).expect("actor ref");
  let watcher = Pid::new(42, 0);

  assert!(RemoteWatchHook::handle_watch(&mut provider, remote.pid(), watcher));
  let watchers = provider.remote_watchers_for_test(remote.pid()).expect("entry");
  assert_eq!(watchers, vec![watcher]);

  assert!(RemoteWatchHook::handle_unwatch(&mut provider, remote.pid(), watcher));
  let watchers = provider.remote_watchers_for_test(remote.pid()).expect("entry");
  assert!(watchers.is_empty());
}

#[test]
fn sender_rejects_quarantined_authority() {
  let system = build_system();
  let provider = provider(&system);
  let remote = provider.actor_ref(remote_path()).expect("actor ref");

  system.state().remote_authority_set_quarantine("127.0.0.1:4100", Some(Duration::from_secs(10)));
  let result = remote.tell(AnyMessageGeneric::new("hello".to_string()));
  assert!(matches!(result, Err(SendError::Closed(_))));
}
