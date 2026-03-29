use crate::core::{Decider, SupervisionStrategy, stream_error::StreamError};

#[test]
fn decider_type_alias_is_callable() {
  fn decider(error: &StreamError) -> SupervisionStrategy {
    match error {
      | StreamError::WouldBlock => SupervisionStrategy::Resume,
      | _ => SupervisionStrategy::Stop,
    }
  }

  let decide: Decider = decider;
  assert_eq!(decide(&StreamError::WouldBlock), SupervisionStrategy::Resume);
  assert_eq!(decide(&StreamError::Failed), SupervisionStrategy::Stop);
}
