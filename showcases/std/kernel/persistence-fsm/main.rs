use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_persistence_core_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentFsm, PersistentRepr, Snapshot, persistent_props, spawn_persistent,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DoorState {
  Closed,
  Open,
}

#[derive(Clone)]
enum DoorCommand {
  Open,
  Pass,
}

#[derive(Clone)]
enum DoorEvent {
  Opened,
  Passed,
}

#[derive(Clone, Copy)]
enum GuardianCommand {
  Start,
  Pass,
}

struct DoorActor {
  context:  PersistenceContext<DoorActor>,
  state:    DoorState,
  passes:   u32,
  observed: SharedLock<(DoorState, u32)>,
}

impl DoorActor {
  fn new(persistence_id: &str, observed: SharedLock<(DoorState, u32)>) -> Self {
    Self { context: PersistenceContext::new(persistence_id.into()), state: DoorState::Closed, passes: 0, observed }
  }

  fn record(&self) {
    self.observed.with_lock(|observed| *observed = (self.state, self.passes));
  }
}

impl Eventsourced for DoorActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<DoorEvent>() {
      self.apply_fsm_event(event);
      match event {
        | DoorEvent::Opened => self.set_fsm_state(DoorState::Open),
        | DoorEvent::Passed => self.set_fsm_state(DoorState::Closed),
      }
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some((state, passes)) = snapshot.data().downcast_ref::<(DoorState, u32)>() {
      self.state = *state;
      self.passes = *passes;
      self.record();
    }
  }

  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<DoorCommand>() {
      match (self.state, command) {
        | (DoorState::Closed, DoorCommand::Open) => {
          self.persist_state_transition(ctx, DoorEvent::Opened, DoorState::Open);
          self.flush_batch(ctx)?;
        },
        | (DoorState::Open, DoorCommand::Pass) => {
          self.persist_state_transition(ctx, DoorEvent::Passed, DoorState::Closed);
          self.flush_batch(ctx)?;
        },
        | _ => {},
      }
    }
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor for DoorActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
    &mut self.context
  }
}

impl PersistentFsm for DoorActor {
  type DomainEvent = DoorEvent;
  type State = DoorState;

  fn apply_fsm_event(&mut self, event: &Self::DomainEvent) {
    if matches!(event, DoorEvent::Passed) {
      self.passes += 1;
    }
    self.record();
  }

  fn set_fsm_state(&mut self, state: Self::State) {
    self.state = state;
    self.record();
  }

  fn fsm_state(&self) -> &Self::State {
    &self.state
  }
}

struct GuardianActor {
  child:    Option<ActorRef>,
  observed: SharedLock<(DoorState, u32)>,
}

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<GuardianCommand>() {
      match command {
        | GuardianCommand::Start => {
          if self.child.is_none() {
            let observed = self.observed.clone();
            let props = persistent_props(move || DoorActor::new("kernel-persistence-fsm-door", observed.clone()));
            let child = spawn_persistent(ctx, &props)
              .map_err(|error| ActorError::recoverable(format!("spawn persistent fsm failed: {error:?}")))?;
            self.child = Some(child);
          }
          if let Some(child) = self.child.as_mut() {
            child
              .try_tell(AnyMessage::new(DoorCommand::Open))
              .map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
          }
        },
        | GuardianCommand::Pass => {
          if let Some(child) = self.child.as_mut() {
            child
              .try_tell(AnyMessage::new(DoorCommand::Pass))
              .map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
          }
        },
      }
    }
    Ok(())
  }
}

fn main() {
  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let observed = SharedLock::new_with_driver::<SpinSyncMutex<_>>((DoorState::Closed, 0_u32));
  let props = Props::from_fn({
    let observed = observed.clone();
    move || GuardianActor { child: None, observed: observed.clone() }
  });
  let config = ActorSystemConfig::new(StdTickDriver::default()).with_extension_installers(installers);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let termination = system.when_terminated();
  let mut guardian = system.user_guardian_ref();

  guardian.tell(AnyMessage::new(GuardianCommand::Start));
  wait_until(|| observed.with_lock(|observed| observed.0 == DoorState::Open));
  guardian.tell(AnyMessage::new(GuardianCommand::Pass));
  wait_until(|| observed.with_lock(|observed| *observed == (DoorState::Closed, 1)));
  let observed_state = observed.with_lock(|observed| *observed);
  println!("kernel_persistence_fsm observed door state: {observed_state:?}");

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
