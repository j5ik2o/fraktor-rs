use super::StreamCompletion;
use crate::core::{Completion, StreamError};

#[test]
fn completion_starts_pending() {
  let completion = StreamCompletion::<u32>::new();
  assert_eq!(completion.poll(), Completion::Pending);
}

#[test]
fn completion_reports_ready_result() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Ok(7));
  assert_eq!(completion.poll(), Completion::Ready(Ok(7)));
}

#[test]
fn completion_try_take_consumes_result() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.try_take(), Some(Err(StreamError::Failed)));
  assert_eq!(completion.poll(), Completion::Pending);
}

#[test]
fn completion_preserves_first_result_on_duplicate_complete() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Ok(42));
  completion.complete(Err(StreamError::Failed));
  assert_eq!(completion.poll(), Completion::Ready(Ok(42)));
}

#[test]
fn completion_preserves_first_error_on_duplicate_complete() {
  let completion: StreamCompletion<u32> = StreamCompletion::new();
  completion.complete(Err(StreamError::Failed));
  completion.complete(Ok(99));
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}
