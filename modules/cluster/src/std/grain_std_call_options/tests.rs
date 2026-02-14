use core::time::Duration;

use crate::{
  core::grain::{GrainCallOptions, GrainRetryPolicy},
  std::{call_options_with_retry, call_options_with_timeout, default_grain_call_options},
};

#[test]
fn default_grain_call_options_matches_core_default() {
  let std_options = default_grain_call_options();
  let core_options = GrainCallOptions::default();
  assert_eq!(std_options, core_options);
}

#[test]
fn call_options_with_timeout_sets_timeout_and_no_retry() {
  let timeout = Duration::from_secs(2);
  let options = call_options_with_timeout(timeout);
  assert_eq!(options.timeout, Some(timeout));
  assert_eq!(options.retry, GrainRetryPolicy::NoRetry);
}

#[test]
fn call_options_with_retry_sets_timeout_and_retry_policy() {
  let timeout = Duration::from_secs(1);
  let retry = GrainRetryPolicy::Fixed { max_retries: 2, delay: Duration::from_millis(50) };
  let options = call_options_with_retry(timeout, retry.clone());
  assert_eq!(options.timeout, Some(timeout));
  assert_eq!(options.retry, retry);
}
