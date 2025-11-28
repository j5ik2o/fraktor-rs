//! Serializes outbound messages and feeds transport queues.

#[cfg(test)]
mod tests;

use alloc::string::String;
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
    QueueError, SyncFifoQueue, SyncQueue,
    backend::{OfferOutcome, OverflowPolicy, VecDequeBackend},
  },
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  endpoint_writer_error::EndpointWriterError, outbound_message::OutboundMessage, outbound_priority::OutboundPriority,
  remoting_envelope::RemotingEnvelope,
};

const DEFAULT_QUEUE_CAPACITY: usize = 128;

/// Shared writer handle protected by the toolbox mutex family.
pub type EndpointWriterShared<TB> =
  ArcShared<<<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<EndpointWriterGeneric<TB>>>;

/// Serializes outbound messages, enforcing priority and backpressure.
pub struct EndpointWriterGeneric<TB: RuntimeToolbox + 'static> {
  #[allow(dead_code)]
  system:        ActorSystemGeneric<TB>,
  serialization: ArcShared<SerializationExtensionGeneric<TB>>,
  system_queue:  SyncFifoQueue<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
  user_queue:    SyncFifoQueue<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>>,
  user_paused:   AtomicBool,
  correlation:   AtomicU64,
  _marker:       PhantomData<TB>,
}

/// Type alias for `EndpointWriterGeneric` with the default `NoStdToolbox`.
pub type EndpointWriter = EndpointWriterGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> EndpointWriterGeneric<TB> {
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
      _marker: PhantomData,
    }
  }

  /// Returns the canonical authority (host[:port]) of the bound actor system when available.
  #[must_use]
  pub fn canonical_authority_components(&self) -> Option<(String, Option<u16>)> {
    self.system.state().canonical_authority_components()
  }

  /// Returns a reference to the underlying actor system.
  #[must_use]
  pub fn system(&self) -> &ActorSystemGeneric<TB> {
    &self.system
  }

  /// Enqueues an outbound message using its declared priority.
  pub fn enqueue(&mut self, message: OutboundMessage<TB>) -> Result<(), EndpointWriterError> {
    let priority = message.priority();
    match priority {
      | OutboundPriority::System => self.offer_system(message),
      | OutboundPriority::User => self.offer_user(message),
    }
  }

  /// Returns the next serialized envelope if available.
  pub fn try_next(&mut self) -> Result<Option<RemotingEnvelope>, EndpointWriterError> {
    if let Some(message) = self.poll_system()? {
      return self.serialize(message, OutboundPriority::System).map(Some);
    }

    if self.user_paused.load(Ordering::Relaxed) {
      return Ok(None);
    }

    if let Some(message) = self.poll_user()? {
      return self.serialize(message, OutboundPriority::User).map(Some);
    }

    Ok(None)
  }

  /// Serializes the outbound message immediately (used by loopback routing).
  pub fn serialize_for_loopback(&self, message: OutboundMessage<TB>) -> Result<RemotingEnvelope, EndpointWriterError> {
    let priority = message.priority();
    self.serialize(message, priority)
  }

  /// Applies the provided backpressure signal.
  pub fn handle_backpressure(&self, signal: BackpressureSignal) {
    match signal {
      | BackpressureSignal::Apply => self.user_paused.store(true, Ordering::Relaxed),
      | BackpressureSignal::Release => self.user_paused.store(false, Ordering::Relaxed),
    }
  }

  fn offer_system(&mut self, message: OutboundMessage<TB>) -> Result<(), EndpointWriterError> {
    match self.system_queue.offer(message) {
      | Ok(OfferOutcome::Enqueued)
      | Ok(OfferOutcome::DroppedNewest { .. })
      | Ok(OfferOutcome::DroppedOldest { .. }) => Ok(()),
      | Ok(OfferOutcome::GrewTo { .. }) => Ok(()),
      | Err(error) => Err(Self::map_offer_error(OutboundPriority::System, error)),
    }
  }

  fn offer_user(&mut self, message: OutboundMessage<TB>) -> Result<(), EndpointWriterError> {
    match self.user_queue.offer(message) {
      | Ok(OfferOutcome::Enqueued)
      | Ok(OfferOutcome::DroppedNewest { .. })
      | Ok(OfferOutcome::DroppedOldest { .. }) => Ok(()),
      | Ok(OfferOutcome::GrewTo { .. }) => Ok(()),
      | Err(error) => Err(Self::map_offer_error(OutboundPriority::User, error)),
    }
  }

  fn poll_system(&mut self) -> Result<Option<OutboundMessage<TB>>, EndpointWriterError> {
    match self.system_queue.poll() {
      | Ok(message) => Ok(Some(message)),
      | Err(QueueError::Empty) => Ok(None),
      | Err(error) => Err(Self::map_poll_error(OutboundPriority::System, error)),
    }
  }

  fn poll_user(&mut self) -> Result<Option<OutboundMessage<TB>>, EndpointWriterError> {
    match self.user_queue.poll() {
      | Ok(message) => Ok(Some(message)),
      | Err(QueueError::Empty) => Ok(None),
      | Err(error) => Err(Self::map_poll_error(OutboundPriority::User, error)),
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

  fn new_queue() -> SyncFifoQueue<OutboundMessage<TB>, VecDequeBackend<OutboundMessage<TB>>> {
    let backend = VecDequeBackend::with_capacity(DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow);
    SyncQueue::new(backend)
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
