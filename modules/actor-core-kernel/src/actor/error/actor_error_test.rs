use super::*;
use crate::actor::messaging::AnyMessage;

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

#[test]
fn typed_constructors_preserve_source_type() {
  let recoverable = ActorError::recoverable_typed::<SendError>("temporary send failure");
  assert!(matches!(recoverable, ActorError::Recoverable(_)));
  assert!(recoverable.is_source_type::<SendError>());
  assert_eq!(recoverable.reason().as_str(), "temporary send failure");

  let fatal = ActorError::fatal_typed::<SendError>("fatal send failure");
  assert!(matches!(fatal, ActorError::Fatal(_)));
  assert!(fatal.is_source_type::<SendError>());
  assert_eq!(fatal.reason().as_str(), "fatal send failure");
}

#[test]
fn from_send_error_formats_send_failure_as_recoverable() {
  let send_error = SendError::closed(AnyMessage::new(5_u32));
  let error = ActorError::from_send_error(&send_error);

  assert!(matches!(error, ActorError::Recoverable(_)));
  assert!(error.reason().as_str().contains("send failed"));
}

#[test]
fn escalate_constructor_preserves_reason_message() {
  // SP-H1: Pekko の defaultDecider における JVM Error 相当を表す `Escalate` variant。
  // `ActorError::escalate` が `Escalate` variant を構築し、理由メッセージが保持されることを確認する。
  let error = ActorError::escalate("boom");
  assert!(matches!(error, ActorError::Escalate(_)));
  assert_eq!(error.reason().as_str(), "boom");
}
