//! Persistent counter example using event sourcing (no_std).
//!
//! Demonstrates `PersistentActor` with event-based state recovery:
//! events are persisted to a journal and replayed on actor restart
//! to restore the counter's accumulated value.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../../../actor/examples/no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::{ActorSystem, ActorSystemConfig},
};
use fraktor_persistence_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentRepr, Snapshot, persistent_props, spawn_persistent,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::SharedAccess};

type TB = NoStdToolbox;

struct Start;

#[derive(Clone)]
enum Command {
  Add(i32),
}

#[derive(Clone)]
enum Event {
  Incremented(i32),
}

struct CounterActor {
  context: PersistenceContext<CounterActor, TB>,
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

impl Eventsourced<TB> for CounterActor {
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

  fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(Command::Add(delta)) = message.downcast_ref::<Command>() {
      self.persist(ctx, Event::Incremented(*delta), |actor, event| actor.apply_event(event));
      self.flush_batch(ctx);
    }
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for CounterActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

struct GuardianActor;

impl Actor<TB> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let props = persistent_props(|| CounterActor::new("counter-1"));
    let child = spawn_persistent(ctx, &props)
      .map_err(|error| ActorError::recoverable(format!("spawn persistent actor failed: {error:?}")))?;
    child.tell(AnyMessage::new(Command::Add(1))).map_err(|_| ActorError::recoverable("send command failed"))?;
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);

  let props = Props::from_fn(|| GuardianActor);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let config = ActorSystemConfig::default().with_tick_driver(tick_driver).with_extension_installers(installers);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
