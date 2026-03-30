use alloc::boxed::Box;

use super::LogLogic;
use crate::core::{FailureAction, FlowLogic, r#impl::StreamError};

#[test]
fn log_logic_passes_elements_through_unchanged() {
  let mut logic = LogLogic::<u32>::new();

  let outputs = logic.apply(Box::new(11_u32)).expect("apply");
  let values: Vec<u32> = outputs.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(values, vec![11_u32]);
}

#[test]
fn log_logic_propagates_failures_without_claiming_handling() {
  let mut logic = LogLogic::<u32>::new();
  assert!(!logic.handles_failures());
  let result = logic.on_failure(StreamError::Failed);
  assert!(matches!(result, Ok(FailureAction::Propagate(StreamError::Failed))));
}
