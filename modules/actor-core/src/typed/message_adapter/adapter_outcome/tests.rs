use crate::typed::message_adapter::{AdapterFailure, AdapterOutcome};

#[test]
fn adapter_outcome_map_transforms_success() {
  let outcome = AdapterOutcome::Converted(2).map(|value| value * 2);
  assert_eq!(outcome, AdapterOutcome::Converted(4));

  let failure = AdapterOutcome::<i32>::Failure(AdapterFailure::Custom("err".into())).map(|value| value);
  assert_eq!(failure, AdapterOutcome::Failure(AdapterFailure::Custom("err".into())));
}
