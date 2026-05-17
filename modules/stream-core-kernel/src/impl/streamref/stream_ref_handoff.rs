use alloc::{borrow::Cow, boxed::Box, collections::VecDeque, format, string::String, vec::Vec};
use core::{marker::PhantomData, num::NonZeroU64};

use fraktor_actor_core_kernel_rs::actor::{actor_ref::ActorRef, messaging::AnyMessage};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{
  stream_ref_endpoint_cleanup::StreamRefEndpointCleanup, stream_ref_endpoint_state::StreamRefEndpointState,
  stream_ref_protocol::StreamRefProtocol,
};
use crate::{
  DynValue, StreamError, downcast_value,
  stage::{CancellationCause, StageActor},
  stream_ref::{StreamRefCumulativeDemand, StreamRefSettings},
};

#[cfg(test)]
#[path = "stream_ref_handoff_test.rs"]
mod tests;

pub(crate) const STREAM_REF_SUBSCRIPTION_TIMEOUT_MESSAGE: &str =
  "remote stream ref partner did not subscribe before the configured timeout";
const LOCAL_STREAM_REF_PARTNER: &str = "local-stream-ref-partner";

struct StreamRefHandoffState<T> {
  values:          VecDeque<StreamRefProtocol>,
  endpoint:        StreamRefEndpointState,
  cleanup:         Option<StreamRefEndpointCleanup>,
  subscribed:      bool,
  closed:          bool,
  failure:         Option<StreamError>,
  buffer_capacity: usize,
  pending_demand:  u64,
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
      endpoint:        StreamRefEndpointState::new(),
      cleanup:         None,
      subscribed:      false,
      closed:          false,
      failure:         None,
      buffer_capacity: StreamRefSettings::new().buffer_capacity(),
      pending_demand:  0,
      next_out_seq_nr: 0,
      next_in_seq_nr:  0,
      _pd:             PhantomData,
    };
    Self { inner: ArcShared::new(SpinSyncMutex::new(state)) }
  }

  fn buffered_value_count(values: &VecDeque<StreamRefProtocol>) -> usize {
    values.iter().filter(|message| matches!(message, StreamRefProtocol::SequencedOnNext { .. })).count()
  }

  fn run_endpoint_cleanup(guard: &mut StreamRefHandoffState<T>) -> Option<StreamError> {
    let cleanup = guard.cleanup.take()?;
    cleanup.run(&mut guard.endpoint).err()
  }

  const fn cancellation_error() -> StreamError {
    StreamError::CancellationCause { cause: CancellationCause::no_more_elements_needed() }
  }

  pub(crate) fn configure_buffer_capacity(&self, buffer_capacity: usize) {
    assert!(buffer_capacity > 0, "stream ref buffer capacity must be greater than zero");
    self.inner.lock().buffer_capacity = buffer_capacity;
  }

  pub(crate) fn attach_endpoint_actor(&self, endpoint_actor: StageActor, partner_actor: Option<ActorRef>) {
    self.inner.lock().cleanup = Some(StreamRefEndpointCleanup::new(endpoint_actor, partner_actor));
  }

  pub(crate) fn drain_endpoint_actor(&self) -> Result<(), StreamError> {
    let endpoint_actor = self.inner.lock().cleanup.as_ref().map(StreamRefEndpointCleanup::endpoint_actor);
    match endpoint_actor {
      | Some(endpoint_actor) => endpoint_actor.drain_pending(),
      | None => Ok(()),
    }
  }

  pub(crate) fn pair_partner_actor(&self, got_ref: String, partner_actor: ActorRef) -> Result<(), StreamError> {
    let mut guard = self.inner.lock();
    guard.endpoint.pair_partner(got_ref)?;
    if let Some(cleanup) = &mut guard.cleanup {
      cleanup.endpoint_actor().watch(&partner_actor)?;
      cleanup.set_partner_actor(partner_actor);
    }
    guard.subscribed = true;
    Ok(())
  }

  pub(crate) fn ensure_partner(&self, got_ref: String) -> Result<(), StreamError> {
    self.inner.lock().endpoint.ensure_partner(got_ref)
  }

  pub(crate) fn subscribe(&self) {
    let mut guard = self.inner.lock();
    if let Err(error) = guard.endpoint.pair_partner(LOCAL_STREAM_REF_PARTNER) {
      guard.failure = Some(error);
      guard.closed = true;
      return;
    }
    debug_assert_eq!(guard.endpoint.partner_ref(), Some(LOCAL_STREAM_REF_PARTNER));
    debug_assert!(guard.endpoint.ensure_partner(LOCAL_STREAM_REF_PARTNER).is_ok());
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

  pub(crate) fn is_terminal(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed || guard.failure.is_some()
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
    guard.endpoint.complete();
    debug_assert!(guard.endpoint.is_completed());
    debug_assert!(guard.endpoint.is_shutdown_requested());
    guard.closed = true;
    seq_nr
  }

  pub(crate) fn cleanup_after_terminal_delivery(&self) -> Result<(), StreamError> {
    let mut guard = self.inner.lock();
    if !guard.endpoint.is_shutdown_requested() {
      return Ok(());
    }
    match Self::run_endpoint_cleanup(&mut guard) {
      | Some(error) => {
        guard.failure = Some(error.clone());
        Err(error)
      },
      | None => Ok(()),
    }
  }

  pub(crate) fn fail(&self, error: StreamError) {
    let _observed = self.fail_and_report(error);
  }

  pub(crate) fn fail_and_report(&self, error: StreamError) -> StreamError {
    let requested_error = error.clone();
    let mut guard = self.inner.lock();
    if guard.failure.is_none() {
      let message = StreamRefProtocol::RemoteStreamFailure { message: Cow::Owned(format!("{error}")) };
      if matches!(message, StreamRefProtocol::RemoteStreamFailure { .. }) {
        guard.values.clear();
      }
      guard.endpoint.fail(error.clone());
      if !guard.endpoint.is_failed() {
        return guard.failure.clone().unwrap_or(requested_error);
      }
      debug_assert!(guard.endpoint.is_failed());
      debug_assert!(guard.endpoint.is_shutdown_requested());
      debug_assert!(guard.endpoint.failure().is_some());
      guard.failure = match Self::run_endpoint_cleanup(&mut guard) {
        | Some(cleanup_error) => Some(StreamError::materialized_resource_rollback_failed(error, cleanup_error)),
        | None => Some(error),
      };
      guard.closed = true;
    }
    guard.failure.clone().unwrap_or(requested_error)
  }

  pub(crate) fn close_for_cancel(&self) {
    let mut guard = self.inner.lock();
    if guard.failure.is_some() {
      return;
    }
    let cancellation_error = Self::cancellation_error();
    guard.endpoint.cancel();
    debug_assert!(guard.endpoint.is_cancelled());
    debug_assert!(guard.endpoint.is_shutdown_requested());
    guard.failure = match Self::run_endpoint_cleanup(&mut guard) {
      | Some(cleanup_error) => {
        Some(StreamError::materialized_resource_rollback_failed(cancellation_error, cleanup_error))
      },
      | None => Some(cancellation_error),
    };
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
    if matches!(guard.values.front(), Some(StreamRefProtocol::SequencedOnNext { .. })) && guard.pending_demand == 0 {
      return Err(StreamError::WouldBlock);
    }
    match guard.values.pop_front() {
      | Some(StreamRefProtocol::SequencedOnNext { seq_nr, payload }) => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
        guard.next_in_seq_nr = guard.next_in_seq_nr.saturating_add(1);
        guard.pending_demand = guard.pending_demand.saturating_sub(1);
        Ok(Some(downcast_value(payload)?))
      },
      | Some(StreamRefProtocol::RemoteStreamCompleted { seq_nr }) => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
        guard.closed = true;
        if let Some(error) = Self::run_endpoint_cleanup(&mut guard) {
          guard.failure = Some(error.clone());
          return Err(error);
        }
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
    let seq_nr = self.inner.lock().next_in_seq_nr;
    self.record_cumulative_demand_from(seq_nr, demand)
  }

  pub(crate) fn record_cumulative_demand_from(&self, seq_nr: u64, demand: NonZeroU64) -> Result<(), StreamError> {
    let mut guard = self.inner.lock();
    let message = StreamRefProtocol::CumulativeDemand { seq_nr, demand };
    match message {
      | StreamRefProtocol::CumulativeDemand { seq_nr, demand } if demand.get() > 0 => {
        StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
        guard.pending_demand = guard.pending_demand.saturating_add(demand.get());
        Ok(())
      },
      | StreamRefProtocol::CumulativeDemand { demand, .. } => {
        Err(StreamError::InvalidDemand { requested: demand.get() })
      },
      | _ => Err(StreamError::Failed),
    }
  }

  pub(crate) fn next_expected_seq_nr(&self) -> u64 {
    self.inner.lock().next_in_seq_nr
  }

  pub(crate) fn send_cumulative_demand_to_partner(&self, seq_nr: u64, demand: NonZeroU64) -> Result<(), StreamError> {
    let (endpoint_actor, mut partner_actor) = {
      let guard = self.inner.lock();
      let Some(cleanup) = &guard.cleanup else {
        return Ok(());
      };
      let Some(partner_actor) = cleanup.partner_actor() else {
        return Ok(());
      };
      (cleanup.endpoint_actor_ref(), partner_actor)
    };
    let message = StreamRefCumulativeDemand::new(seq_nr, demand);
    partner_actor
      .try_tell(AnyMessage::new(message).with_sender(endpoint_actor))
      .map_err(|error| StreamError::from_send_error(&error))
  }

  pub(crate) fn enqueue_remote_element(&self, seq_nr: u64, value: T) -> Result<(), StreamError>
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
    guard.values.push_back(StreamRefProtocol::SequencedOnNext { seq_nr, payload: Box::new(value) as DynValue });
    Ok(())
  }

  pub(crate) fn enqueue_remote_completed(&self, seq_nr: u64) -> Result<(), StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    if !guard.closed {
      guard.values.push_back(StreamRefProtocol::RemoteStreamCompleted { seq_nr });
      guard.endpoint.complete();
      guard.closed = true;
    }
    Ok(())
  }

  pub(crate) fn enqueue_remote_failure(&self, message: String) {
    self.fail(StreamError::failed_with_context(message));
  }

  pub(crate) fn drain_ready_protocols(&self) -> Result<Vec<StreamRefProtocol>, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }

    let mut messages = Vec::new();
    loop {
      if matches!(guard.values.front(), Some(StreamRefProtocol::SequencedOnNext { .. })) && guard.pending_demand == 0 {
        break;
      }
      let Some(message) = guard.values.pop_front() else {
        break;
      };
      match message {
        | StreamRefProtocol::SequencedOnNext { seq_nr, payload } => {
          StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
          guard.next_in_seq_nr = guard.next_in_seq_nr.saturating_add(1);
          guard.pending_demand = guard.pending_demand.saturating_sub(1);
          messages.push(StreamRefProtocol::SequencedOnNext { seq_nr, payload });
        },
        | StreamRefProtocol::RemoteStreamCompleted { seq_nr } => {
          StreamRefProtocol::validate_sequence(guard.next_in_seq_nr, seq_nr)?;
          guard.closed = true;
          messages.push(StreamRefProtocol::RemoteStreamCompleted { seq_nr });
          break;
        },
        | StreamRefProtocol::RemoteStreamFailure { message } => {
          let error = StreamError::failed_with_context(message.clone());
          guard.failure = Some(error);
          guard.closed = true;
          messages.push(StreamRefProtocol::RemoteStreamFailure { message });
          break;
        },
        | StreamRefProtocol::CumulativeDemand { .. }
        | StreamRefProtocol::OnSubscribeHandshake
        | StreamRefProtocol::Ack => {
          return Err(StreamError::Failed);
        },
      }
    }
    Ok(messages)
  }

  pub(crate) fn has_pending_protocols(&self) -> bool {
    !self.inner.lock().values.is_empty()
  }

  pub(crate) const fn subscription_timeout_error() -> StreamError {
    StreamError::StreamRefSubscriptionTimeout { message: Cow::Borrowed(STREAM_REF_SUBSCRIPTION_TIMEOUT_MESSAGE) }
  }
}
