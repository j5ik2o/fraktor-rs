use core::time::Duration;

use fraktor_actor_core_rs::{
  actor::{
    ActorContext, Pid,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
  },
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};

use crate::core::{
  eventsourced::Eventsourced, persistent_repr::PersistentRepr, recovery::Recovery,
  recovery_timed_out::RecoveryTimedOut, snapshot::Snapshot,
};

struct DummyEventsourced {
  persistence_id: String,
  last:           u64,
}

impl Eventsourced for DummyEventsourced {
  fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.last
  }
}

#[test]
fn eventsourced_default_recovery_is_latest() {
  let dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0 };

  let recovery = dummy.recovery();
  assert_eq!(recovery, Recovery::default());
  assert_eq!(dummy.recovery_event_timeout(), Duration::from_secs(30));
}

#[test]
fn eventsourced_default_hooks_do_not_panic() {
  let mut dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0 };
  let system = ActorSystem::from_state(SystemStateShared::new(SystemState::new()));
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContext::new(&system, pid);
  let message = AnyMessage::new(1_i32);

  let _ = dummy.receive_command(&mut ctx, message.as_view());
  dummy.on_recovery_timed_out(&RecoveryTimedOut::new("pid-1"));
}
