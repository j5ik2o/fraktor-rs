use crate::typed::message_adapter::AdapterFailure;

#[test]
fn adapter_failure_debug() {
  let failure = AdapterFailure::Custom("oops".into());
  let formatted = alloc::format!("{:?}", failure);
  assert!(formatted.contains("Custom"));
}
