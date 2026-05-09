use super::AdapterLifecycleState;

#[test]
fn lifecycle_state_reports_alive_flag() {
  let state = AdapterLifecycleState::new();
  assert!(state.is_alive());
  state.mark_stopped();
  assert!(!state.is_alive());
}
