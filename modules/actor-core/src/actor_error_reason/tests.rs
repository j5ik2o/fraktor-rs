use alloc::string::String;

use crate::actor_error_reason::ActorErrorReason;

#[test]
fn reason_conversions_work() {
  let owned = ActorErrorReason::from(String::from("owned"));
  assert_eq!(owned.as_str(), "owned");

  let borrowed = ActorErrorReason::from("borrowed");
  assert_eq!(borrowed.as_str(), "borrowed");
}
