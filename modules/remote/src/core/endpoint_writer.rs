//! Serializes outbound messages and feeds transport queues.

#[cfg(test)]
mod tests;

use core::{
  marker::PhantomData,
  sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use fraktor_actor_rs::core::{
  event_stream::BackpressureSignal,
  serialization::{SerializationCallScope, SerializationExtensionGeneric},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  collections::queue::{
    QueueError, SyncFifoQueueShared, SyncQueue,
    backend::{OfferOutcome, OverflowPolicy, VecDequeBackend},
  },
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use crate::core::{
  endpoint_writer_error::EndpointWriterError, outbound_message::OutboundMessage,
  outbound_priority::OutboundPriority, remoting_envelope::RemotingEnvelope,
};

const DEFAULT_QUEUE_CAPACITY: usize = 128;

/// Serializes outbound messages, enforcing priority and backpressure.
pub struct EndpointWriter<TB: RuntimeToolbox + 'static> {
  #[allow(dead_code)]
  system:        ActorSystemGeneric<TB>,
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
  system_queue:  SyncFifoQueueShared<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
  user_queue:    SyncFifoQueueShared<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
  user_paused:   AtomicBool,
  correlation:   AtomicU64,
  _marker:       PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> EndpointWriter<TB> {
  /// Creates a writer bound to the provided actor system and serialization extension.
  #[must_use]
  pub fn new(system: ActorSystemGeneric<TB>, serialization: ArcShared<SerializationExtensionGeneric<TB>>) -> Self {
    Self {
      system,
      serialization,
      system_queue: Self::new_queue(),
      user_queue: Self::new_queue(),
      user_paused: AtomicBool::new(false),
      correlation: AtomicU64::new(1),
      _marker:     PhantomData,
    }
  }

  /// Enqueues an outbound message using its declared priority.
  pub fn enqueue(&self, message: OutboundMessage<TB>) -> Result<(), EndpointWriterError> {
    let priority = message.priority();
    match priority {
      | OutboundPriority::System => self.offer(&self.system_queue, message, priority),
      | OutboundPriority::User => self.offer(&self.user_queue, message, priority),
    }
  }

  /// Returns the next serialized envelope if available.
  pub fn try_next(&self) -> Result<Option<RemotingEnvelope>, EndpointWriterError> {
    if let Some(message) = self.poll_queue(&self.system_queue, OutboundPriority::System)? {
      return self.serialize(message, OutboundPriority::System).map(Some);
    }

    if self.user_paused.load(Ordering::Relaxed) {
      return Ok(None);
    }

    if let Some(message) = self.poll_queue(&self.user_queue, OutboundPriority::User)? {
      return self.serialize(message, OutboundPriority::User).map(Some);
    }

    Ok(None)
  }

  /// Applies the provided backpressure signal.
  pub fn handle_backpressure(&self, signal: BackpressureSignal) {
    match signal {
      | BackpressureSignal::Apply => self.user_paused.store(true, Ordering::Relaxed),
      | BackpressureSignal::Release => self.user_paused.store(false, Ordering::Relaxed),
    }
  }

  fn offer(
    &self,
    queue: &SyncFifoQueueShared<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
    message: OutboundMessage<TB>,
    priority: OutboundPriority,
  ) -> Result<(), EndpointWriterError> {
    match queue.offer(message) {
      | Ok(OfferOutcome::Enqueued) | Ok(OfferOutcome::DroppedNewest { .. }) | Ok(OfferOutcome::DroppedOldest { .. }) => {
        Ok(())
      },
      | Ok(OfferOutcome::GrewTo { .. }) => Ok(()),
      | Err(error) => Err(Self::map_offer_error(priority, error)),
    }
  }

  fn poll_queue(
    &self,
    queue: &SyncFifoQueueShared<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
    priority: OutboundPriority,
  ) -> Result<Option<OutboundMessage<TB>>, EndpointWriterError> {
    match queue.poll() {
      | Ok(message) => Ok(Some(message)),
      | Err(QueueError::Empty) => Ok(None),
      | Err(error) => Err(Self::map_poll_error(priority, error)),
    }
  }

  fn serialize(
    &self,
    message: OutboundMessage<TB>,
    priority: OutboundPriority,
  ) -> Result<RemotingEnvelope, EndpointWriterError> {
    let (payload, recipient, remote_node, reply_to) = message.into_parts();
    let serialized = self
      .serialization
      .serialize(payload.payload(), SerializationCallScope::Remote)
      .map_err(EndpointWriterError::Serialization)?;
    let correlation_id = self.next_correlation_id();
    Ok(RemotingEnvelope::new(recipient, remote_node, reply_to, serialized, correlation_id, priority))
  }

  fn next_correlation_id(&self) -> fraktor_actor_rs::core::event_stream::CorrelationId {
    let value = self.correlation.fetch_add(1, Ordering::Relaxed) as u128;
    fraktor_actor_rs::core::event_stream::CorrelationId::from_u128(value)
  }

  fn new_queue() -> SyncFifoQueueShared<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>> {
    let backend = VecDequeBackend::with_capacity(DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow);
    let queue = SyncQueue::new(backend);
    let mutex = SpinSyncMutex::new(queue);
    SyncFifoQueueShared::new(ArcShared::new(mutex))
  }

  fn map_offer_error(priority: OutboundPriority, error: QueueError<OutboundMessage<TB>>) -> EndpointWriterError {
    match error {
      | QueueError::Full(_) => EndpointWriterError::QueueFull(priority),
      | QueueError::Closed(_) | QueueError::Disconnected => EndpointWriterError::QueueClosed(priority),
      | QueueError::AllocError(_) => EndpointWriterError::QueueUnavailable { priority, reason: "alloc failure" },
      | QueueError::OfferError(_) => EndpointWriterError::QueueUnavailable { priority, reason: "offer error" },
      | QueueError::TimedOut(_) | QueueError::WouldBlock | QueueError::Empty => {
        EndpointWriterError::QueueUnavailable { priority, reason: "offer interrupted" }
      },
    }
  }

  fn map_poll_error(priority: OutboundPriority, error: QueueError<OutboundMessage<TB>>) -> EndpointWriterError {
    match error {
      | QueueError::Full(_) => EndpointWriterError::QueueFull(priority),
      | QueueError::Closed(_) | QueueError::Disconnected => EndpointWriterError::QueueClosed(priority),
      | QueueError::AllocError(_) => EndpointWriterError::QueueUnavailable { priority, reason: "alloc failure" },
      | QueueError::OfferError(_) => EndpointWriterError::QueueUnavailable { priority, reason: "offer error" },
      | QueueError::TimedOut(_) | QueueError::WouldBlock => {
        EndpointWriterError::QueueUnavailable { priority, reason: "poll interrupted" }
      },
      | QueueError::Empty => EndpointWriterError::QueueUnavailable { priority, reason: "unexpected empty queue" },
    }
  }
}
