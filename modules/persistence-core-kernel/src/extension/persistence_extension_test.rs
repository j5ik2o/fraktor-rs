use alloc::{boxed::Box, vec::Vec};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::{ActorError, SendError},
    extension::ExtensionId,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex};

use crate::{
  config::PersistenceSettings,
  extension::{PersistenceExtension, PersistenceExtensionId},
  journal::{InMemoryJournal, JournalActorConfig, PersistencePluginProxyCommand},
  snapshot::{InMemorySnapshotStore, SnapshotActorConfig},
};

type MessageStore = ArcShared<SpinSyncMutex<Vec<AnyMessage>>>;

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn actor_ref_with_store(pid: Pid) -> (ActorRef, MessageStore) {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    SpinSyncMutex<Box<dyn ActorRefSender>>,
  >(Box::new(TestSender { messages: messages.clone() })));
  (ActorRef::new(pid, sender), messages)
}

impl PersistenceExtension {
  const fn new_with_actor_refs(
    journal_actor: ActorRef,
    snapshot_actor: ActorRef,
    settings: PersistenceSettings,
  ) -> Self {
    Self { journal_actor, snapshot_actor, settings }
  }
}

#[test]
fn persistence_extension_creates_actor_refs() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();

  let extension = PersistenceExtension::new(&system, journal, snapshot).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor_ref().pid(), extension.snapshot_actor_ref().pid());

  let cloned = extension.clone();
  assert_eq!(cloned.journal_actor_ref().pid(), extension.journal_actor_ref().pid());
}

#[test]
fn persistence_extension_accepts_explicit_runtime_settings() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));

  let extension =
    PersistenceExtension::new_with_settings(&system, journal, snapshot, settings).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_eq!(extension.settings().journal_actor_config(), JournalActorConfig::new(2));
  assert_eq!(extension.settings().snapshot_actor_config(), SnapshotActorConfig::new(3));
}

#[test]
fn persistence_extension_creates_proxy_actor_refs() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");

  let extension = PersistenceExtension::new_proxy(&system).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor_ref().pid(), extension.snapshot_actor_ref().pid());
}

#[test]
fn persistence_extension_sends_plugin_target_location_commands() {
  let (journal_proxy, journal_commands) = actor_ref_with_store(Pid::new(20_000, 1));
  let (snapshot_proxy, snapshot_commands) = actor_ref_with_store(Pid::new(20_001, 1));
  let (journal_target, _journal_target_store) = actor_ref_with_store(Pid::new(20_002, 1));
  let (snapshot_target, _snapshot_target_store) = actor_ref_with_store(Pid::new(20_003, 1));
  let mut extension =
    PersistenceExtension::new_with_actor_refs(journal_proxy, snapshot_proxy, PersistenceSettings::default());

  extension.set_plugin_target_location(journal_target.clone(), snapshot_target.clone()).expect("target location");

  let journal_commands = journal_commands.lock();
  assert_eq!(journal_commands.len(), 1);
  let command = journal_commands[0].payload().downcast_ref::<PersistencePluginProxyCommand>().expect("journal command");
  assert!(matches!(command, PersistencePluginProxyCommand::SetJournalTarget { target } if target == &journal_target));

  let snapshot_commands = snapshot_commands.lock();
  assert_eq!(snapshot_commands.len(), 1);
  let command =
    snapshot_commands[0].payload().downcast_ref::<PersistencePluginProxyCommand>().expect("snapshot command");
  assert!(matches!(command, PersistencePluginProxyCommand::SetSnapshotTarget { target } if target == &snapshot_target));
}

#[test]
fn persistence_extension_id_creates_shared_extension() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let extension_id = PersistenceExtensionId::new(InMemoryJournal::new(), InMemorySnapshotStore::new());

  let extension = extension_id.create_extension(&system);

  let journal_pid = extension.with_read(|inner| inner.journal_actor_ref().pid());
  let snapshot_pid = extension.with_read(|inner| inner.snapshot_actor_ref().pid());
  assert_ne!(journal_pid, Pid::new(0, 0));
  assert_ne!(snapshot_pid, Pid::new(0, 0));
}

#[test]
#[should_panic(expected = "persistence extension bootstrap failed")]
fn persistence_extension_id_panics_when_runtime_actor_names_are_already_taken() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let extension_id = PersistenceExtensionId::new(InMemoryJournal::new(), InMemorySnapshotStore::new());

  drop(extension_id.create_extension(&system));
  drop(extension_id.create_extension(&system));
}
