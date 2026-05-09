use alloc::{borrow::Cow, string::String};

use crate::actor::error::actor_error_reason::ActorErrorReason;

#[test]
fn reason_conversions_work() {
  let owned = ActorErrorReason::from(String::from("owned"));
  assert_eq!(owned.as_str(), "owned");

  let borrowed = ActorErrorReason::from("borrowed");
  assert_eq!(borrowed.as_str(), "borrowed");

  let cow = ActorErrorReason::from(Cow::Borrowed("cow"));
  assert_eq!(cow.as_str(), "cow");
}
