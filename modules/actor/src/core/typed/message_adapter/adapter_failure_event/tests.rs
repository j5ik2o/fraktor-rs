use crate::core::{
  actor_prim::Pid,
  typed::message_adapter::{AdapterFailure, AdapterFailureEvent},
};

#[test]
fn adapter_failure_event_exposes_fields() {
  let event = AdapterFailureEvent::new(Pid::new(1, 0), AdapterFailure::Custom("oops".into()));
  assert_eq!(event.pid(), Pid::new(1, 0));
  assert!(matches!(event.failure(), AdapterFailure::Custom(message) if message == "oops"));
}
