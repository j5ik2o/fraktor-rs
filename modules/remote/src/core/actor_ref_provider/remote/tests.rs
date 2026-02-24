#![cfg(any(test, feature = "test-support"))]

use alloc::{boxed::Box, string::String};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric, Pid,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationExtensionSharedGeneric, SerializationSetup,
    SerializationSetupBuilder, Serializer, SerializerId, builtin::StringSerializer,
  },
  system::{ActorSystemConfig, ActorSystemGeneric, remote::RemoteWatchHook},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess},
};

use crate::core::{
  actor_ref_provider::remote::RemoteActorRefProvider,
  endpoint_writer::{EndpointWriter, EndpointWriterShared},
  remoting_extension::{RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingExtensionConfig},
  transport::{LoopbackTransport, RemoteTransport, RemoteTransportShared, TransportBind},
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
) -> SerializationExtensionSharedGeneric<NoStdToolbox> {
  SerializationExtensionSharedGeneric::new(SerializationExtensionGeneric::new(system, serialization_setup()))
}

fn provider(system: &ActorSystemGeneric<NoStdToolbox>) -> RemoteActorRefProvider {
  let serialization = serialization_extension(system);
  let writer = EndpointWriterShared::new(EndpointWriter::new(system.downgrade(), serialization));
  let control_handle = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let control: RemotingControlShared<NoStdToolbox> =
    ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(control_handle));
  let mut transport = LoopbackTransport::<NoStdToolbox>::default();
  transport.spawn_listener(&TransportBind::new("127.0.0.1", Some(4100))).expect("bind 127.0.0.1:4100");
  transport.spawn_listener(&TransportBind::new("10.0.0.1", Some(9000))).expect("bind 10.0.0.1:9000");
  control.lock().register_remote_transport_shared(RemoteTransportShared::new(Box::new(transport)));
  control.lock().start().expect("control start");
  RemoteActorRefProvider::from_components(system.clone(), writer, control).expect("provider builds")
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
  let mut provider = provider(&system);
  let writer = provider.writer_for_test();
  let remote = provider.actor_ref(remote_path()).expect("actor ref");

  remote.tell(AnyMessageGeneric::new("hello".to_string())).expect("send succeeds");

  let envelope = writer.with_write(|w| w.try_next()).expect("poll writer").expect("envelope exists");
  assert_eq!(envelope.recipient().to_relative_string(), "/user/user/svc");
  assert_eq!(envelope.remote_node().system(), "remote-app");
  assert_eq!(envelope.remote_node().host(), "127.0.0.1");
  assert_eq!(envelope.remote_node().port(), Some(4100));
}

#[test]
fn watch_remote_associates_authority() {
  let system = build_system();
  let mut provider = provider(&system);
  let mut parts = ActorPathParts::with_authority("remote-app", Some(("10.0.0.1", 9000)));
  parts = parts.with_guardian(GuardianKind::User);
  provider.watch_remote(parts).expect("watch succeeds");
}

#[test]
fn registers_remote_entry_for_remote_pid() {
  let system = build_system();
  let mut provider = provider(&system);
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
  let mut provider = provider(&system);
  let remote = provider.actor_ref(remote_path()).expect("actor ref");

  system.state().remote_authority_set_quarantine("127.0.0.1:4100", Some(Duration::from_secs(10)));
  let result = remote.tell(AnyMessageGeneric::new("hello".to_string()));
  assert!(matches!(result, Err(SendError::Closed(_))));
}
