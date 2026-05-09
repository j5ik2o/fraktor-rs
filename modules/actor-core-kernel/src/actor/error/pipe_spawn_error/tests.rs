use super::PipeSpawnError;

#[test]
fn target_stopped_helper_matches_target_stopped_variant() {
  assert!(!PipeSpawnError::ActorUnavailable.is_target_stopped());
  assert!(PipeSpawnError::TargetStopped.is_target_stopped());
}

#[test]
fn display_matches_public_contract() {
  assert_eq!(alloc::format!("{}", PipeSpawnError::ActorUnavailable), "actor cell is unavailable");
  assert_eq!(alloc::format!("{}", PipeSpawnError::TargetStopped), "actor stopped before pipe task completed");
}
