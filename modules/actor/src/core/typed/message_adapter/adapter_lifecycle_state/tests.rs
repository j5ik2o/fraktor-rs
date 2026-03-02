use crate::core::typed::message_adapter::adapter_lifecycle_state::AdapterLifecycleState;

#[test]
fn lifecycle_state_reports_alive_flag() {
  let state = AdapterLifecycleState::new();
  assert!(state.is_alive());
  state.mark_stopped();
  assert!(!state.is_alive());
}
