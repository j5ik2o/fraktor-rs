#![cfg(any(test, feature = "test-support"))]

use alloc::{boxed::Box, string::String};
use core::time::Duration;

use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext, Address, Pid,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
    actor_ref::ActorRef,
    actor_ref_provider::ActorRefProvider,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    setup::ActorSystemConfig,
  },
  serialization::{
    SerializationCallScope, SerializationExtension, SerializationExtensionShared, SerializationSetup,
    SerializationSetupBuilder, Serializer, SerializerId, builtin::StringSerializer,
  },
  system::{ActorSystem, remote::RemoteWatchHook},
};
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use crate::core::{
  actor_ref_provider::remote::RemoteActorRefProvider,
  endpoint_writer::{EndpointWriter, EndpointWriterShared},
  remoting_extension::{RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingExtensionConfig},
  transport::{LoopbackTransport, RemoteTransport, RemoteTransportShared, TransportBind},
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| NoopActor).with_name("provider-tests");
  let remoting = fraktor_actor_rs::core::kernel::system::remote::RemotingConfig::default()
    .with_canonical_host("127.0.0.1")
    .with_canonical_port(4100);
  let system_config = ActorSystemConfig::default()
    .with_remoting_config(remoting)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystem::new_with_config(&props, &system_config).expect("system builds")
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

fn serialization_extension(system: &ActorSystem) -> SerializationExtensionShared {
  SerializationExtensionShared::new(SerializationExtension::new(system, serialization_setup()))
}

fn provider(system: &ActorSystem) -> RemoteActorRefProvider {
  let serialization = serialization_extension(system);
  let writer = EndpointWriterShared::new(EndpointWriter::new(system.downgrade(), serialization));
  let control_handle = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let control: RemotingControlShared = ArcShared::new(RuntimeMutex::new(control_handle));
  let mut transport = LoopbackTransport::default();
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
  let mut remote = provider.actor_ref(remote_path()).expect("actor ref");

  remote.tell(AnyMessage::new("hello".to_string()));

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
fn tell_to_quarantined_authority_records_dead_letter() {
  let system = build_system();
  let mut provider = provider(&system);
  let mut remote = provider.actor_ref(remote_path()).expect("actor ref");

  system.state().remote_authority_set_quarantine("127.0.0.1:4100", Some(Duration::from_secs(10)));
  remote.tell(AnyMessage::new("hello".to_string()));

  let queued = provider.writer_for_test().with_write(|writer| writer.try_next()).expect("writer");
  assert!(queued.is_none());
  assert_eq!(system.state().dead_letters().len(), 1);
}

#[test]
fn remote_provider_exposes_classic_contract_helpers() {
  let system = build_system();
  let provider = provider(&system);

  let default = provider.get_default_address().expect("default address");
  assert_eq!(default, Address::remote(system.name(), "127.0.0.1", 4100));
  assert_eq!(provider.get_external_address_for(&Address::remote("peer", "10.0.0.1", 9000)), Some(default.clone()));
  assert!(provider.root_guardian_at(&default).is_some());
  assert!(provider.deployer().is_some());

  let temp_path = provider.temp_path_with_prefix("probe").expect("prefixed temp path");
  assert!(temp_path.to_relative_string().starts_with("/user/temp/probe-"));
  let temp_container = provider.temp_container().expect("temp container");
  assert_eq!(temp_container.path().expect("temp path").to_relative_string(), "/user/temp");

  let temp_ref = ActorRef::new(Pid::new(7878, 0), fraktor_actor_rs::core::kernel::actor::actor_ref::NullSender);
  let temp_name = provider.register_temp_actor(temp_ref).expect("temp actor");
  provider.unregister_temp_actor_path(&provider.temp_path().child(&temp_name)).expect("unregister temp actor path");
  assert!(provider.temp_actor(&temp_name).is_none());

  let future = provider.termination_future();
  assert!(!future.with_read(|inner| inner.is_ready()));
  system.state().mark_terminated();
  assert!(future.with_read(|inner| inner.is_ready()));
}
