use crate::{QueueOfferResult, r#impl::StreamError};

#[test]
fn queue_offer_result_distinguishes_variants() {
  assert_ne!(QueueOfferResult::Enqueued, QueueOfferResult::Dropped);
  assert_ne!(QueueOfferResult::Dropped, QueueOfferResult::QueueClosed);
}

#[test]
fn queue_offer_result_failure_keeps_error() {
  let result = QueueOfferResult::Failure(StreamError::BufferOverflow);
  assert_eq!(result, QueueOfferResult::Failure(StreamError::BufferOverflow));
}
