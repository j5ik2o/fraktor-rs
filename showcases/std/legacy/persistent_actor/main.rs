//! Persistent actor with event sourcing.
//!
//! Demonstrates `PersistentActor` with event-based state recovery:
//! events are persisted to an in-memory journal and replayed on actor restart
//! to restore the counter's accumulated value.
//!
//! Run with: `cargo run -p fraktor-showcases-std --features advanced --example persistent_actor`

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  event::logging::LogLevel,
  system::ActorSystem,
};
use fraktor_persistence_core_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentRepr, Snapshot, persistent_props, spawn_persistent,
};
use fraktor_showcases_std::subscribe_kernel_tracing_logger;
// --- メッセージ定義 ---

struct Start;

#[derive(Clone)]
enum Command {
  Add(i32),
}

#[derive(Clone)]
enum Event {
  Incremented(i32),
}

// --- 永続カウンターアクター ---

struct CounterActor {
  context: PersistenceContext<CounterActor>,
  value:   i32,
}

impl CounterActor {
  fn new(persistence_id: &str) -> Self {
    Self { context: PersistenceContext::new(persistence_id.into()), value: 0 }
  }

  fn apply_event(&mut self, event: &Event) {
    let Event::Incremented(delta) = event;
    self.value += delta;
  }
}

impl Eventsourced for CounterActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<Event>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i32>() {
      self.value = *value;
    }
  }

  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(Command::Add(delta)) = message.downcast_ref::<Command>() {
      ctx.log(LogLevel::Info, format!("command: Add({delta}), current value: {}", self.value));
      self.persist(ctx, Event::Incremented(*delta), |actor, event| actor.apply_event(event));
      self.flush_batch(ctx)?;
      ctx.log(LogLevel::Info, format!("after persist: value = {}", self.value));
    }
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor for CounterActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
    &mut self.context
  }
}

// --- Guardian アクター ---

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let props = persistent_props(|| CounterActor::new("counter-1"));
    let mut child = spawn_persistent(ctx, &props)
      .map_err(|error| ActorError::recoverable(format!("spawn persistent actor failed: {error:?}")))?;

    child.try_tell(AnyMessage::new(Command::Add(1))).map_err(|_| ActorError::recoverable("send Add(1) failed"))?;
    child.try_tell(AnyMessage::new(Command::Add(5))).map_err(|_| ActorError::recoverable("send Add(5) failed"))?;
    child.try_tell(AnyMessage::new(Command::Add(3))).map_err(|_| ActorError::recoverable("send Add(3) failed"))?;
    Ok(())
  }
}

// --- エントリーポイント ---

fn main() {
  use std::thread;

  println!("=== Persistent actor with event sourcing ===");

  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = ExtensionInstallers::default().with_extension_installer(installer);

  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(StdTickDriver::default()).with_extension_installers(installers);

  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let _log_subscription = subscribe_kernel_tracing_logger(&system);
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  // コマンド処理と flush_batch 完了を待機してからシャットダウン
  thread::sleep(std::time::Duration::from_millis(500));

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
