use alloc::boxed::Box;

use super::{LogLogic, LogObservation, LogObservationHandle};
use crate::core::{FailureAction, FlowLogic, StreamError};

impl LogObservationHandle {
  fn snapshot(&self) -> LogObservation {
    *self.inner.lock()
  }
}

#[test]
fn log_logic_records_elements_completion_and_failures() {
  let observation = LogObservationHandle::new();
  let mut logic = LogLogic::<u32>::new(observation.clone());

  let outputs = logic.apply(Box::new(11_u32)).expect("apply");
  let values: Vec<u32> = outputs.into_iter().map(|value| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(values, vec![11_u32]);
  assert_eq!(observation.snapshot(), LogObservation { element_count: 1, completed: false, failure_count: 0 });

  logic.on_source_done().expect("source done");
  assert_eq!(observation.snapshot(), LogObservation { element_count: 1, completed: true, failure_count: 0 });

  let result = logic.on_failure(StreamError::Failed);
  assert!(matches!(result, Ok(FailureAction::Propagate(StreamError::Failed))));
  assert_eq!(observation.snapshot(), LogObservation { element_count: 1, completed: true, failure_count: 1 });
}
