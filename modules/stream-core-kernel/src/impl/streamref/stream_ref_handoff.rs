use alloc::{borrow::Cow, boxed::Box, collections::VecDeque, format};
use core::{marker::PhantomData, num::NonZeroU64};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::stream_ref_protocol::StreamRefProtocol;
use crate::{DynValue, StreamError, downcast_value, stream_ref::StreamRefSettings};

#[cfg(test)]
#[path = "stream_ref_handoff_test.rs"]
mod tests;

pub(crate) const STREAM_REF_SUBSCRIPTION_TIMEOUT_MESSAGE: &str =
  "remote stream ref partner did not subscribe before the configured timeout";

struct StreamRefHandoffState<T> {
  values:          VecDeque<StreamRefProtocol>,
  subscribed:      bool,
  closed:          bool,
  failure:         Option<StreamError>,
  buffer_capacity: usize,
  next_out_seq_nr: u64,
  next_in_seq_nr:  u64,
  _pd:             PhantomData<fn() -> T>,
}

/// Shared local handoff state between two stream-reference endpoints.
pub(crate) struct StreamRefHandoff<T> {
  inner: ArcShared<SpinSyncMutex<StreamRefHandoffState<T>>>,
}

impl<T> Clone for StreamRefHandoff<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> StreamRefHandoff<T> {
  pub(crate) fn new() -> Self {
    let state = StreamRefHandoffState {
      values:          VecDeque::new(),
      subscribed:      false,
      closed:          false,
      failure:         None,
      buffer_capacity: StreamRefSettings::new().buffer_capacity(),
      next_out_seq_nr: 0,
      next_in_seq_nr:  0,
      _pd:             PhantomData,
    };
    Self { inner: ArcShared::new(SpinSyncMutex::new(state)) }
  }

  fn buffered_value_count(values: &VecDeque<StreamRefProtocol>) -> usize {
    values.iter().filter(|message| matches!(message, StreamRefProtocol::SequencedOnNext { .. })).count()
  }

  pub(crate) fn configure_buffer_capacity(&self, buffer_capacity: usize) {
    assert!(buffer_capacity > 0, "stream ref buffer capacity must be greater than zero");
    self.inner.lock().buffer_capacity = buffer_capacity;
  }

  pub(crate) fn subscribe(&self) {
    let mut guard = self.inner.lock();
    let handshake = StreamRefProtocol::OnSubscribeHandshake;
    let ack = StreamRefProtocol::Ack;
    if matches!(handshake, StreamRefProtocol::OnSubscribeHandshake) && matches!(ack, StreamRefProtocol::Ack) {
      guard.subscribed = true;
    }
  }

  pub(crate) fn is_subscribed(&self) -> bool {
    let guard = self.inner.lock();
    guard.subscribed
  }

  pub(crate) fn offer(&self, value: T) -> Result<u64, StreamError>
  where
    T: Send + 'static, {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    if guard.closed {
      return Err(StreamError::StreamDetached);
    }
    if Self::buffered_value_count(&guard.values) >= guard.buffer_capacity {
      return Err(StreamError::BufferOverflow);
    }
    let seq_nr = guard.next_out_seq_nr;
    guard.next_out_seq_nr = guard.next_out_seq_nr.saturating_add(1);
    guard.values.push_back(StreamRefProtocol::SequencedOnNext { seq_nr, payload: Box::new(value) as DynValue });
    Ok(seq_nr)
  }

  pub(crate) fn complete(&self) -> u64 {
    let mut guard = self.inner.lock();
    let seq_nr = guard.next_out_seq_nr;
    guard.values.push_back(StreamRefProtocol::RemoteStreamCompleted { seq_nr });
    guard.closed = true;
    seq_nr
  }

  pub(crate) fn fail(&self, error: StreamError) {
    let mut guard = self.inner.lock();
    if guard.failure.is_none() {
      let message = StreamRefProtocol::RemoteStreamFailure { message: Cow::Owned(format!("{error}")) };
      if matches!(message, StreamRefProtocol::RemoteStreamFailure { .. }) {
        guard.values.clear();
      }
      guard.failure = Some(error);
      guard.closed = true;
    }
  }

  pub(crate) fn close_for_cancel(&self) {
    let mut guard = self.inner.lock();
    if guard.failure.is_some() {
      return;
    }
    guard.closed = true;
    guard.values.clear();
  }

  pub(crate) fn poll_or_drain(&self) -> Result<Option<T>, StreamError>
  where
    T: Send + 'static, {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    match guard.values.pop_front() {
      | Some(StreamRefProtocol::SequencedOnNext { seq_nr, payload }) => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
        guard.next_in_seq_nr = guard.next_in_seq_nr.saturating_add(1);
        Ok(Some(downcast_value(payload)?))
      },
      | Some(StreamRefProtocol::RemoteStreamCompleted { seq_nr }) => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
        guard.closed = true;
        Ok(None)
      },
      | Some(StreamRefProtocol::RemoteStreamFailure { message }) => {
        let error = StreamError::failed_with_context(message);
        guard.failure = Some(error.clone());
        guard.closed = true;
        Err(error)
      },
      | Some(
        StreamRefProtocol::CumulativeDemand { .. } | StreamRefProtocol::OnSubscribeHandshake | StreamRefProtocol::Ack,
      ) => Err(StreamError::Failed),
      | None if guard.closed => Ok(None),
      | None => Err(StreamError::WouldBlock),
    }
  }

  pub(crate) fn record_cumulative_demand(&self) -> Result<(), StreamError> {
    let Some(demand) = NonZeroU64::new(1) else {
      return Err(StreamError::InvalidDemand { requested: 0 });
    };
    let guard = self.inner.lock();
    let message = StreamRefProtocol::CumulativeDemand { seq_nr: guard.next_in_seq_nr, demand };
    match message {
      | StreamRefProtocol::CumulativeDemand { seq_nr, demand } if demand.get() > 0 => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)
      },
      | StreamRefProtocol::CumulativeDemand { demand, .. } => {
        Err(StreamError::InvalidDemand { requested: demand.get() })
      },
      | _ => Err(StreamError::Failed),
    }
  }

  pub(crate) const fn subscription_timeout_error() -> StreamError {
    StreamError::StreamRefSubscriptionTimeout { message: Cow::Borrowed(STREAM_REF_SUBSCRIPTION_TIMEOUT_MESSAGE) }
  }
}
