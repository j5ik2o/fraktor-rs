use fraktor_actor_rs::core::{
  actor::{ActorContextGeneric, Pid},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  system::{
    ActorSystemGeneric,
    state::{SystemStateSharedGeneric, system_state::SystemStateGeneric},
  },
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  eventsourced::Eventsourced, persistent_repr::PersistentRepr, recovery::Recovery, snapshot::Snapshot,
};

struct DummyEventsourced {
  persistence_id: String,
  last:           u64,
}

impl Eventsourced<NoStdToolbox> for DummyEventsourced {
  fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
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
}

#[test]
fn eventsourced_default_hooks_do_not_panic() {
  let mut dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0 };
  let system = ActorSystemGeneric::<NoStdToolbox>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let message = AnyMessageGeneric::new(1_i32);

  let _ = dummy.receive_command(&mut ctx, message.as_view());
}
