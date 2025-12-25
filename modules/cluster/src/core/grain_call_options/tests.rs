use crate::core::{GrainCallOptions, GrainRetryPolicy};

#[test]
fn default_options_use_no_timeout_and_no_retry() {
  let options = GrainCallOptions::default();
  assert_eq!(options.timeout, None);
  assert_eq!(options.retry, GrainRetryPolicy::NoRetry);
}
