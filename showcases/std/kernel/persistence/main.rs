use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_persistence_core_kernel_rs::{
  extension::PersistenceExtensionInstaller,
  journal::InMemoryJournal,
  persistent::{Eventsourced, PersistenceContext, PersistentActor, PersistentRepr, persistent_props, spawn_persistent},
  snapshot::{InMemorySnapshotStore, Snapshot},
};
use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};

struct Start;

#[derive(Clone)]
struct Add(i32);

#[derive(Clone)]
struct Added(i32);

struct CounterActor {
  context:  PersistenceContext<CounterActor>,
  value:    i32,
  observed: SharedLock<i32>,
}

impl CounterActor {
  fn new(persistence_id: &str, observed: SharedLock<i32>) -> Self {
    Self { context: PersistenceContext::new(persistence_id.into()), value: 0, observed }
  }

  fn apply_event(&mut self, event: &Added) {
    self.value += event.0;
    self.observed.with_lock(|observed| *observed = self.value);
  }
}

impl Eventsourced for CounterActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<Added>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i32>() {
      self.value = *value;
      self.observed.with_lock(|observed| *observed = self.value);
    }
  }

  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<Add>() {
      self.persist(ctx, Added(command.0), |actor, event| actor.apply_event(event));
      self.flush_batch(ctx)?;
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

struct GuardianActor {
  observed: SharedLock<i32>,
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let observed = self.observed.clone();
    let props = persistent_props(move || CounterActor::new("kernel-persistence-counter", observed.clone()));
    let mut child = spawn_persistent(ctx, &props)
      .map_err(|error| ActorError::recoverable(format!("spawn persistent failed: {error:?}")))?;
    for delta in [1_i32, 5, 3] {
      child.try_tell(AnyMessage::new(Add(delta))).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    }
    Ok(())
  }
}

fn main() {
  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let observed = SharedLock::new_with_driver::<SpinSyncMutex<_>>(0_i32);
  let props = Props::from_fn({
    let observed = observed.clone();
    move || GuardianActor { observed: observed.clone() }
  });
  let config = ActorSystemConfig::new(StdTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start));
  wait_until(|| observed.with_lock(|value| *value == 9));
  let observed_value = observed.with_lock(|value| *value);
  println!("kernel_persistence replayed counter value: {observed_value}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..2_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
