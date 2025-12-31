use crate::core::persistent_actor_state::PersistentActorState;

#[test]
fn persistent_actor_state_valid_transitions() {
  let state = PersistentActorState::WaitingRecoveryPermit;
  let state = state.transition_to_recovery_started().expect("transition failed");
  let state = state.transition_to_recovering().expect("transition failed");
  let state = state.transition_to_processing_commands().expect("transition failed");
  let state = state.transition_to_persisting_events().expect("transition failed");
  let state = state.transition_to_processing_commands().expect("transition failed");

  assert_eq!(state, PersistentActorState::ProcessingCommands);
}

#[test]
fn persistent_actor_state_invalid_transition() {
  let state = PersistentActorState::WaitingRecoveryPermit;

  assert!(state.transition_to_processing_commands().is_err());
}
