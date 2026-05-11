#[cfg(test)]
#[path = "stream_ref_protocol_test.rs"]
mod tests;

use alloc::borrow::Cow;
use core::num::NonZeroU64;

use crate::{DynValue, StreamError};

pub(crate) const INVALID_SEQUENCE_NUMBER_MESSAGE: &str = "invalid stream ref sequence number";

pub(crate) enum StreamRefProtocol {
  SequencedOnNext { seq_nr: u64, payload: DynValue },
  CumulativeDemand { seq_nr: u64, demand: NonZeroU64 },
  OnSubscribeHandshake,
  RemoteStreamCompleted { seq_nr: u64 },
  RemoteStreamFailure { message: Cow<'static, str> },
  Ack,
}

impl StreamRefProtocol {
  pub(crate) const fn validate_sequence(expected_seq_nr: u64, got_seq_nr: u64) -> Result<(), StreamError> {
    if expected_seq_nr == got_seq_nr {
      return Ok(());
    }
    Err(StreamError::InvalidSequenceNumber {
      expected_seq_nr,
      got_seq_nr,
      message: Cow::Borrowed(INVALID_SEQUENCE_NUMBER_MESSAGE),
    })
  }
}
