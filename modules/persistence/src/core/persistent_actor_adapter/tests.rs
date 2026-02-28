use alloc::{string::ToString, vec::Vec};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorCellGeneric, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric, message_invoker::MessageInvokerPipelineGeneric},
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{
    ActorSystemConfigGeneric, ActorSystemGeneric,
    state::{SystemStateSharedGeneric, system_state::SystemStateGeneric},
  },
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  eventsourced::Eventsourced, in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  journal_error::JournalError, journal_response::JournalResponse, persistence_context::PersistenceContext,
  persistence_error::PersistenceError, persistence_extension_installer::PersistenceExtensionInstaller,
  persistent_actor::PersistentActor, persistent_actor_adapter::PersistentActorAdapter,
  persistent_actor_state::PersistentActorState, persistent_repr::PersistentRepr, recovery::Recovery,
  recovery_timed_out::RecoveryTimedOut, snapshot::Snapshot, snapshot_response::SnapshotResponse,
  stash_overflow_strategy::StashOverflowStrategy,
};

type TB = NoStdToolbox;

struct NoopActor;

impl Actor<TB> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct DummyPersistentActor {
  context:                 PersistenceContext<DummyPersistentActor, TB>,
  timed_out_count:         usize,
  recovery_failure_count:  usize,
  last_recovery_failure:   Option<PersistenceError>,
  command_count:           usize,
  command_log:             Vec<i32>,
  stash_overflow_strategy: StashOverflowStrategy,
  stash_capacity:          usize,
}

impl DummyPersistentActor {
  fn new() -> Self {
    Self {
      context:                 PersistenceContext::new("pid-1".to_string()),
      timed_out_count:         0,
      recovery_failure_count:  0,
      last_recovery_failure:   None,
      command_count:           0,
      command_log:             Vec::new(),
      stash_overflow_strategy: StashOverflowStrategy::Fail,
      stash_capacity:          1024,
    }
  }

  fn with_stash_settings(stash_overflow_strategy: StashOverflowStrategy, stash_capacity: usize) -> Self {
    Self { stash_overflow_strategy, stash_capacity, ..Self::new() }
  }
}

impl Eventsourced<TB> for DummyPersistentActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn on_recovery_timed_out(&mut self, _signal: &RecoveryTimedOut) {
    self.timed_out_count = self.timed_out_count.saturating_add(1);
  }

  fn on_recovery_failure(&mut self, cause: &PersistenceError) {
    self.recovery_failure_count = self.recovery_failure_count.saturating_add(1);
    self.last_recovery_failure = Some(cause.clone());
  }

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    self.command_count = self.command_count.saturating_add(1);
    if let Some(value) = message.downcast_ref::<i32>() {
      self.command_log.push(*value);
    }
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for DummyPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }

  fn stash_overflow_strategy(&self) -> StashOverflowStrategy {
    self.stash_overflow_strategy
  }

  fn stash_capacity(&self) -> usize {
    self.stash_capacity
  }
}

struct MismatchPersistentActor {
  context: PersistenceContext<MismatchPersistentActor, TB>,
}

impl MismatchPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-other".to_string()) }
  }
}

impl Eventsourced<TB> for MismatchPersistentActor {
  fn persistence_id(&self) -> &str {
    "pid-1"
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for MismatchPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

fn build_context(system: &ActorSystemGeneric<TB>) -> ActorContextGeneric<'static, TB> {
  let pid = system.allocate_pid();
  let props = PropsGeneric::from_fn(|| NoopActor);
  let cell =
    ActorCellGeneric::create(system.state(), pid, None, "test".into(), &props).expect("actor cell should be created");
  system.state().register_cell(cell);
  ActorContextGeneric::new(system, pid)
}

fn register_noop_actor_ref(system: &ActorSystemGeneric<TB>, name: &str) -> ActorRefGeneric<TB> {
  let pid = system.allocate_pid();
  let props = PropsGeneric::from_fn(|| NoopActor);
  let cell =
    ActorCellGeneric::create(system.state(), pid, None, name.into(), &props).expect("actor cell should be created");
  system.state().register_cell(cell.clone());
  cell.actor_ref()
}

fn prepare_processing_commands(
  adapter: &mut PersistentActorAdapter<DummyPersistentActor, TB>,
  ctx: &mut ActorContextGeneric<'_, TB>,
) {
  prepare_recovery_started(adapter, ctx);
  let _ = adapter.actor.persistence_context().handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRefGeneric::null(),
  );
  let _ = adapter
    .actor
    .persistence_context()
    .handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });
}

fn prepare_recovery_started(
  adapter: &mut PersistentActorAdapter<DummyPersistentActor, TB>,
  ctx: &mut ActorContextGeneric<'_, TB>,
) {
  let journal_ref = register_noop_actor_ref(ctx.system(), "journal");
  let snapshot_ref = register_noop_actor_ref(ctx.system(), "snapshot");
  adapter.actor.persistence_context().bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  adapter
    .actor
    .persistence_context()
    .start_recovery(Recovery::default(), ActorRefGeneric::null())
    .expect("start recovery");
}

fn prepare_recovering(
  adapter: &mut PersistentActorAdapter<DummyPersistentActor, TB>,
  ctx: &mut ActorContextGeneric<'_, TB>,
) {
  prepare_recovery_started(adapter, ctx);
  let _ = adapter.actor.persistence_context().handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRefGeneric::null(),
  );
}

fn prepare_stashing_commands(
  adapter: &mut PersistentActorAdapter<DummyPersistentActor, TB>,
  ctx: &mut ActorContextGeneric<'_, TB>,
) {
  prepare_processing_commands(adapter, ctx);
  adapter.actor.persist(ctx, 1_i32, |_actor, _event| {});
  adapter.actor.flush_batch(ctx).expect("flush batch");
  assert!(adapter.actor.persistence_context().should_stash_commands());
}

#[test]
fn adapter_pre_start_fails_without_extension() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  let result = adapter.pre_start(&mut ctx);
  assert!(matches!(result, Err(ActorError::Fatal(_))));
}

#[test]
fn adapter_pre_start_binds_context() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  adapter.pre_start(&mut ctx).expect("pre_start");

  let result = adapter.actor.persistence_context().bind_actor_refs(ActorRefGeneric::null(), ActorRefGeneric::null());
  assert!(result.is_err());
}

#[test]
fn adapter_pre_start_schedules_recovery_timeout() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  adapter.pre_start(&mut ctx).expect("pre_start");

  assert!(adapter.recovery_timeout_handle.is_some());
}

#[test]
fn adapter_pre_start_rejects_persistence_id_mismatch() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = MismatchPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  let result = adapter.pre_start(&mut ctx);
  assert!(matches!(result, Err(ActorError::Fatal(_))));
}

#[test]
fn adapter_rearms_recovery_timeout_on_snapshot_and_replayed_message() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  adapter.pre_start(&mut ctx).expect("pre_start");

  let snapshot_handle_raw = adapter.recovery_timeout_handle.as_ref().expect("snapshot handle").raw();

  let snapshot_response = AnyMessageGeneric::<TB>::new(SnapshotResponse::LoadSnapshotResult {
    snapshot:       None,
    to_sequence_nr: u64::MAX,
  });
  adapter.receive(&mut ctx, snapshot_response.as_view()).expect("snapshot response");
  let replay_handle_raw = adapter.recovery_timeout_handle.as_ref().expect("replay handle").raw();
  assert_ne!(snapshot_handle_raw, replay_handle_raw);

  let replayed = AnyMessageGeneric::<TB>::new(JournalResponse::ReplayedMessage {
    persistent_repr: PersistentRepr::new("pid-1", 1, ArcShared::new(11_i32)),
  });
  adapter.receive(&mut ctx, replayed.as_view()).expect("replayed message");
  let replayed_handle_raw = adapter.recovery_timeout_handle.as_ref().expect("rearmed replay handle").raw();
  assert_ne!(replay_handle_raw, replayed_handle_raw);
}

#[test]
fn adapter_forwards_recovery_timed_out_signal() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  let message = fraktor_actor_rs::core::messaging::AnyMessageGeneric::<TB>::new(RecoveryTimedOut::new("pid-1"));

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.timed_out_count, 1);
}

#[test]
fn adapter_recovery_tick_triggers_timeout_while_waiting_snapshot() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovery_started(&mut adapter, &mut ctx);
  let message = AnyMessageGeneric::<TB>::new(super::RecoveryTick::waiting_snapshot(adapter.recovery_timeout_epoch));

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(matches!(result, Err(ActorError::Fatal(_))));
  assert_eq!(adapter.actor.timed_out_count, 1);
  assert_eq!(adapter.actor.recovery_failure_count, 1);
  assert!(matches!(
    adapter.actor.last_recovery_failure.as_ref(),
    Some(PersistenceError::Recovery(reason)) if reason.contains("waiting for snapshot")
  ));
}

#[test]
fn adapter_recovery_tick_triggers_timeout_while_waiting_event() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovering(&mut adapter, &mut ctx);
  let message = AnyMessageGeneric::<TB>::new(super::RecoveryTick::waiting_event(adapter.recovery_timeout_epoch));

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(matches!(result, Err(ActorError::Fatal(_))));
  assert_eq!(adapter.actor.timed_out_count, 1);
  assert_eq!(adapter.actor.recovery_failure_count, 1);
  assert!(matches!(
    adapter.actor.last_recovery_failure.as_ref(),
    Some(PersistenceError::Recovery(reason)) if reason.contains("waiting for event")
  ));
}

#[test]
fn adapter_ignores_stale_recovery_tick_epoch() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  adapter.pre_start(&mut ctx).expect("pre_start");

  let snapshot_response = AnyMessageGeneric::<TB>::new(SnapshotResponse::LoadSnapshotResult {
    snapshot:       None,
    to_sequence_nr: u64::MAX,
  });
  adapter.receive(&mut ctx, snapshot_response.as_view()).expect("snapshot response");
  let stale_epoch = adapter.recovery_timeout_epoch;

  let replayed = AnyMessageGeneric::<TB>::new(JournalResponse::ReplayedMessage {
    persistent_repr: PersistentRepr::new("pid-1", 1, ArcShared::new(11_i32)),
  });
  adapter.receive(&mut ctx, replayed.as_view()).expect("replayed message");
  assert_ne!(stale_epoch, adapter.recovery_timeout_epoch);

  let stale_tick = AnyMessageGeneric::<TB>::new(super::RecoveryTick::waiting_event(stale_epoch));
  adapter.receive(&mut ctx, stale_tick.as_view()).expect("stale tick should be ignored");

  assert_eq!(adapter.actor.timed_out_count, 0);
}

#[test]
fn adapter_propagates_unstash_all_error() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_processing_commands(&mut adapter, &mut ctx);
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesSuccessful {
    instance_id: adapter.actor.persistence_context().instance_id(),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "actor cell unavailable during unstash")
  );
}

#[test]
fn adapter_stashes_command_until_defer_completes_after_persist_unfenced() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_processing_commands(&mut adapter, &mut ctx);
  adapter.actor.persist_unfenced(&mut ctx, 1_i32, |_actor, _event| {});
  adapter.actor.defer(&mut ctx, 2_i32, |_actor, _event| {});
  adapter.actor.flush_batch(&mut ctx).expect("flush");
  assert!(adapter.actor.persistence_context().should_stash_commands());
  let instance_id = adapter.actor.persistence_context().instance_id();

  let write_success = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessageSuccess {
    repr: PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)),
    instance_id,
  });
  adapter.receive(&mut ctx, write_success.as_view()).expect("write success");
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
  assert!(adapter.actor.persistence_context().should_stash_commands());

  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(123_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  assert_eq!(adapter.actor.command_count, 0);

  let completion = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesSuccessful { instance_id });
  adapter.receive(&mut ctx, completion.as_view()).expect("completion");
  assert!(!adapter.actor.persistence_context().should_stash_commands());
}

#[test]
fn adapter_stashes_command_between_write_message_success_and_write_messages_successful() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_processing_commands(&mut adapter, &mut ctx);
  adapter.actor.persist(&mut ctx, 1_i32, |_actor, _event| {});
  adapter.actor.flush_batch(&mut ctx).expect("flush");
  assert!(adapter.actor.persistence_context().should_stash_commands());
  let instance_id = adapter.actor.persistence_context().instance_id();

  let write_success = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessageSuccess {
    repr: PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)),
    instance_id,
  });
  adapter.receive(&mut ctx, write_success.as_view()).expect("write success");
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
  assert!(adapter.actor.persistence_context().should_stash_commands());

  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let boundary_command = AnyMessageGeneric::<TB>::new(125_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, boundary_command).expect("boundary command should be stashed");
  assert_eq!(adapter.actor.command_count, 0);

  let write_messages_successful =
    AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesSuccessful { instance_id });
  adapter.receive(&mut ctx, write_messages_successful.as_view()).expect("write messages successful should unstash");
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::ProcessingCommands);
  assert!(!adapter.actor.persistence_context().should_stash_commands());
  assert_eq!(adapter.actor.command_count, 0);
  assert!(adapter.actor.command_log.is_empty());

  let follow_up_command = AnyMessageGeneric::<TB>::new(126_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, follow_up_command).expect("follow-up command should be processed");
  assert_eq!(adapter.actor.command_count, 1);
  assert_eq!(adapter.actor.command_log, vec![126_i32]);
}

#[test]
fn adapter_does_not_stash_command_during_persist_all_async() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_processing_commands(&mut adapter, &mut ctx);
  adapter.actor.persist_all_async(&mut ctx, vec![1_i32, 2_i32], |_actor, _event| {});
  adapter.actor.flush_batch(&mut ctx).expect("flush");
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
  assert!(!adapter.actor.persistence_context().should_stash_commands());

  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(124_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be processed");

  assert_eq!(adapter.actor.command_count, 1);
  assert_eq!(adapter.actor.command_log, vec![124_i32]);
}

#[test]
fn adapter_stashes_command_during_recovery_started() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovery_started(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(123_i32);

  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");

  assert_eq!(adapter.actor.command_count, 0);
}

#[test]
fn adapter_stashes_command_during_recovering() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovering(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(456_i32);

  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");

  assert_eq!(adapter.actor.command_count, 0);
}

#[test]
fn adapter_unstash_all_on_recovery_success() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovering(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(789_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "actor cell unavailable during unstash")
  );
}

#[test]
fn adapter_unstash_all_on_highest_sequence_nr() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovering(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(790_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::HighestSequenceNr {
    persistence_id: "pid-1".to_string(),
    sequence_nr:    0,
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "actor cell unavailable during unstash")
  );
}

#[test]
fn adapter_stops_on_replay_messages_failure_during_recovery() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovering(&mut adapter, &mut ctx);
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::ReplayMessagesFailure {
    cause: JournalError::ReadFailed("replay failed".to_string()),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(matches!(
    result,
    Err(ActorError::Fatal(reason)) if reason.as_str().contains("persistent actor stopped after replay failure")
  ));
  assert_eq!(adapter.actor.recovery_failure_count, 1);
}

#[test]
fn adapter_stops_on_highest_sequence_nr_failure_during_recovery() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_recovery_started(&mut adapter, &mut ctx);
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::HighestSequenceNrFailure {
    persistence_id: "pid-1".to_string(),
    cause:          JournalError::ReadFailed("highest sequence lookup failed".to_string()),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(matches!(
    result,
    Err(ActorError::Fatal(reason))
      if reason.as_str().contains("persistent actor stopped after highest sequence number lookup failure")
  ));
  assert_eq!(adapter.actor.recovery_failure_count, 1);
}

#[test]
fn adapter_stops_on_write_message_failure() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(791_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  let instance_id = adapter.actor.persistence_context().instance_id();
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessageFailure {
    repr: PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)),
    cause: JournalError::WriteFailed("write failed".to_string()),
    instance_id,
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(matches!(
    result,
    Err(ActorError::Fatal(reason)) if reason.as_str().contains("persistent actor stopped after write failure")
  ));
}

#[test]
fn adapter_ignores_write_message_failure_when_instance_id_mismatches() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let mismatched_instance_id = adapter.actor.persistence_context().instance_id().wrapping_add(1);
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessageFailure {
    repr:        PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)),
    cause:       JournalError::WriteFailed("write failed".to_string()),
    instance_id: mismatched_instance_id,
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
}

#[test]
fn adapter_unstash_all_on_write_message_rejected() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(792_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let instance_id = adapter.actor.persistence_context().instance_id();
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessageRejected {
    repr: PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32)),
    cause: JournalError::WriteFailed("write rejected".to_string()),
    instance_id,
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "actor cell unavailable during unstash")
  );
}

#[test]
fn adapter_keeps_stash_on_write_messages_failed_with_positive_write_count() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(793_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 1,
    instance_id: adapter.actor.persistence_context().instance_id(),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
}

#[test]
fn adapter_unstash_all_on_write_messages_failed_with_zero_write_count() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(794_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 0,
    instance_id: adapter.actor.persistence_context().instance_id(),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "actor cell unavailable during unstash")
  );
}

#[test]
fn adapter_ignores_write_messages_successful_when_instance_id_mismatches() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(795_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesSuccessful {
    instance_id: adapter.actor.persistence_context().instance_id().wrapping_add(1),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
}

#[test]
fn adapter_ignores_stale_write_messages_successful_after_processing_resumes() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_processing_commands(&mut adapter, &mut ctx);
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesSuccessful {
    instance_id: adapter.actor.persistence_context().instance_id().wrapping_add(1),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::ProcessingCommands);
}

#[test]
fn adapter_ignores_write_messages_failed_with_zero_write_count_when_instance_id_mismatches() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(796_i32);
  pipeline.invoke_user(&mut adapter, &mut ctx, command).expect("command should be stashed");
  ctx.system().state().remove_cell(&ctx.pid());
  let message = AnyMessageGeneric::<TB>::new(JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 0,
    instance_id: adapter.actor.persistence_context().instance_id().wrapping_add(1),
  });

  let result = adapter.receive(&mut ctx, message.as_view());

  assert!(result.is_ok());
  assert_eq!(adapter.actor.persistence_context().state(), PersistentActorState::PersistingEvents);
}

#[test]
fn adapter_applies_drop_strategy_only_on_stash_overflow() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::with_stash_settings(StashOverflowStrategy::Drop, 0);
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(123_i32);

  let result = pipeline.invoke_user(&mut adapter, &mut ctx, command);

  assert!(result.is_ok());
}

#[test]
fn adapter_applies_fail_strategy_on_stash_overflow() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::with_stash_settings(StashOverflowStrategy::Fail, 0);
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let pipeline = MessageInvokerPipelineGeneric::<TB>::new();
  let command = AnyMessageGeneric::<TB>::new(123_i32);

  let result = pipeline.invoke_user(&mut adapter, &mut ctx, command);
  let error = result.expect_err("overflow should fail when strategy is Fail");

  assert!(ActorContextGeneric::<TB>::is_stash_overflow_error(&error));
}

#[test]
fn adapter_does_not_apply_drop_strategy_to_non_overflow_stash_error() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::with_stash_settings(StashOverflowStrategy::Drop, 0);
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);
  prepare_stashing_commands(&mut adapter, &mut ctx);
  let command = AnyMessageGeneric::<TB>::new(321_i32);

  let result = adapter.receive(&mut ctx, command.as_view());

  assert!(
    matches!(result, Err(ActorError::Recoverable(reason)) if reason.as_str() == "stash requires an active user message")
  );
}
