use alloc::borrow::Cow;

use super::{DUPLICATE_MATERIALIZATION_MESSAGE, INVALID_PARTNER_MESSAGE, StreamRefEndpointState};
use crate::StreamError;

#[test]
fn pair_partner_accepts_first_partner() {
  let mut state = StreamRefEndpointState::new();

  assert_eq!(state.pair_partner("actor-a"), Ok(()));

  assert_eq!(state.partner_ref(), Some("actor-a"));
  assert_eq!(state.ensure_partner("actor-a"), Ok(()));
}

#[test]
fn pair_partner_rejects_double_materialization() {
  let mut state = StreamRefEndpointState::new();
  state.pair_partner("actor-a").expect("first partner");

  let error = state.pair_partner("actor-b").expect_err("second partner must fail");

  assert_eq!(error, StreamError::InvalidPartnerActor {
    expected_ref: Cow::Borrowed("actor-a"),
    got_ref:      Cow::Borrowed("actor-b"),
    message:      Cow::Borrowed(DUPLICATE_MATERIALIZATION_MESSAGE),
  });
}

#[test]
fn ensure_partner_rejects_non_partner_messages() {
  let mut state = StreamRefEndpointState::new();
  state.pair_partner("actor-a").expect("first partner");

  let error = state.ensure_partner("actor-b").expect_err("non-partner must fail");

  assert_eq!(error, StreamError::InvalidPartnerActor {
    expected_ref: Cow::Borrowed("actor-a"),
    got_ref:      Cow::Borrowed("actor-b"),
    message:      Cow::Borrowed(INVALID_PARTNER_MESSAGE),
  });
}

#[test]
fn ensure_partner_requires_initial_pairing() {
  let state = StreamRefEndpointState::new();

  assert_eq!(state.ensure_partner("actor-a"), Err(StreamError::StreamRefTargetNotInitialized));
}

#[test]
fn completion_requests_endpoint_shutdown() {
  let mut state = StreamRefEndpointState::new();

  state.complete();
  state.fail(StreamError::Failed);

  assert!(state.is_completed());
  assert!(!state.is_failed());
  assert!(state.is_shutdown_requested());
  assert_eq!(state.failure(), None);
}

#[test]
fn cancellation_requests_endpoint_shutdown() {
  let mut state = StreamRefEndpointState::new();

  state.cancel();

  assert!(state.is_cancelled());
  assert!(state.is_shutdown_requested());
}

#[test]
fn failure_requests_endpoint_shutdown_and_preserves_error() {
  let mut state = StreamRefEndpointState::new();

  state.fail(StreamError::Failed);
  state.complete();

  assert!(state.is_failed());
  assert!(!state.is_completed());
  assert!(state.is_shutdown_requested());
  assert_eq!(state.failure(), Some(&StreamError::Failed));
}
