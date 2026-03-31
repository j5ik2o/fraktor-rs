use crate::core::kernel::actor::{
  error::ActorError,
  messaging::{AnyMessage, Status},
};

#[test]
fn status_success_keeps_payload() {
  let status = Status::success(AnyMessage::new(7_u32));
  match status {
    | Status::Success(payload) => {
      let value = payload.payload().downcast_ref::<u32>().expect("u32");
      assert_eq!(*value, 7);
    },
    | Status::Failure(_) => panic!("expected success"),
  }
}

#[test]
fn status_failure_keeps_error() {
  let status = Status::failure(ActorError::recoverable_typed::<u32>("boom"));
  match status {
    | Status::Failure(error) => {
      assert_eq!(error.reason().as_str(), "boom");
      assert!(error.is_source_type::<u32>());
    },
    | Status::Success(_) => panic!("expected failure"),
  }
}
