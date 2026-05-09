use crate::dsl::StatusReplyError;

#[test]
fn should_store_message() {
  let err = StatusReplyError::new("something went wrong");
  assert_eq!(err.message(), "something went wrong");
}

#[test]
fn should_display_message() {
  let err = StatusReplyError::new("failure");
  let display = alloc::format!("{}", err);
  assert_eq!(display, "failure");
}
