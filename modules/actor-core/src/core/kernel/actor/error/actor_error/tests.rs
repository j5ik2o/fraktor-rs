use super::*;

#[test]
fn recoverable_and_fatal_transform() {
  let recoverable = ActorError::recoverable("temporary");
  assert!(matches!(recoverable, ActorError::Recoverable(_)));

  let fatal = recoverable.clone().into_fatal();
  assert!(matches!(fatal, ActorError::Fatal(_)));
  assert_eq!(fatal.reason().as_str(), "temporary");

  let back = fatal.into_recoverable();
  assert!(matches!(back, ActorError::Recoverable(_)));
}

#[test]
fn accepts_custom_reason() {
  let reason = ActorErrorReason::new("custom");
  let error = ActorError::fatal(reason.clone());
  assert_eq!(error.reason(), &reason);
}
