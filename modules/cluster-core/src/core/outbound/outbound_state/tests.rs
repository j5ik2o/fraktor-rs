use super::OutboundState;

#[test]
fn quarantine_is_blocking() {
  let state = OutboundState::Quarantine { reason: "invalid association".to_string(), deadline: None };
  assert!(state.is_blocking());
}

#[test]
fn connected_is_not_blocking() {
  let state = OutboundState::Connected;
  assert!(!state.is_blocking());
}
