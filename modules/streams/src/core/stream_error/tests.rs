use super::StreamError;

#[test]
fn error_messages_are_stable() {
  assert_eq!(StreamError::NotStarted.to_string(), "materializer not started");
  assert_eq!(StreamError::InvalidDemand.to_string(), "invalid demand request");
  assert_eq!(StreamError::ExecutorUnavailable.to_string(), "executor is unavailable");
}
