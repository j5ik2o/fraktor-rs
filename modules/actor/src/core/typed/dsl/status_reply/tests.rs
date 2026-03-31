use crate::core::typed::dsl::StatusReply;

#[test]
fn success_should_return_value() {
  let reply = StatusReply::success(42);
  assert!(reply.is_success());
  assert!(!reply.is_error());
  assert_eq!(reply.into_result().unwrap(), 42);
}

#[test]
fn error_should_return_err() {
  let reply = StatusReply::<u32>::error("bad");
  assert!(reply.is_error());
  assert!(!reply.is_success());
  let err = reply.into_result().unwrap_err();
  assert_eq!(err.message(), "bad");
}

#[test]
fn ack_should_be_unit_success() {
  let reply = StatusReply::<()>::ack();
  assert!(reply.is_success());
  assert!(reply.into_result().is_ok());
}
