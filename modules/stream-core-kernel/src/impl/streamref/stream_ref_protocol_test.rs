use alloc::borrow::Cow;

use super::StreamRefProtocol;
use crate::{DynValue, StreamError};

#[test]
fn sequenced_on_next_uses_zero_based_sequence_and_preserves_payload() {
  // Given: seq_nr=0 の最初の要素
  let payload: DynValue = Box::new(42_u32);

  // When: SequencedOnNext protocol message を作る
  let message = StreamRefProtocol::SequencedOnNext { seq_nr: 0, payload };

  // Then: sequence は 0 始まりで、payload は失われない
  let StreamRefProtocol::SequencedOnNext { seq_nr, payload } = message else {
    panic!("expected SequencedOnNext");
  };
  assert_eq!(seq_nr, 0);
  assert_eq!(*payload.downcast::<u32>().expect("u32 payload"), 42_u32);
}

#[test]
fn validate_sequence_reports_expected_and_actual_sequence_numbers() {
  // Given: expected=3 に対して got=2 の out-of-order message
  let expected = 3;
  let got = 2;

  // When: sequence 検証を行う
  let error = StreamRefProtocol::validate_sequence(expected, got).expect_err("sequence mismatch");

  // Then: Pekko InvalidSequenceNumberException 相当の情報を保持する
  assert_eq!(error, StreamError::InvalidSequenceNumber {
    expected_seq_nr: expected,
    got_seq_nr:      got,
    message:         Cow::Borrowed("invalid stream ref sequence number"),
  });
}
