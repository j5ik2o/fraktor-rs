use core::mem::size_of;

use crate::{
  StateSourcedEffector, StateSourcedEffectorSignal, state_sourced_effector_signal_auth::StateSourcedEffectorSignalAuth,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregateCommand {
  Signal(StateSourcedEffectorSignal<u32>),
}

#[test]
fn effector_handle_type_is_available() {
  let _message = AggregateCommand::Signal(StateSourcedEffectorSignal::RecoveryCompleted {
    auth:     StateSourcedEffectorSignalAuth::new(),
    state:    Some(1),
    revision: 1,
  });

  assert!(size_of::<StateSourcedEffector<u32, AggregateCommand>>() > 0);
}
