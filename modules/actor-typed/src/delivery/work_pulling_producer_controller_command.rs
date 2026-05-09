//! Commands accepted by the work-pulling producer controller actor.

#[cfg(test)]
mod tests;

use crate::{
  TypedActorRef,
  delivery::{
    DurableProducerQueueState, MessageSent, ProducerControllerRequestNext, StoreMessageSentAck,
    WorkPullingProducerControllerRequestNext, WorkerStats,
  },
  receptionist::Listing,
};

/// Commands handled by
/// [`WorkPullingProducerController`](crate::delivery::WorkPullingProducerController).
///
/// User code constructs commands through
/// [`WorkPullingProducerController`](crate::delivery::WorkPullingProducerController)
/// factory methods. Internal protocol messages are crate-private.
#[derive(Clone)]
pub struct WorkPullingProducerControllerCommand<A>(pub(crate) WorkPullingProducerControllerCommandKind<A>)
where
  A: Clone + Send + Sync + 'static;

#[derive(Clone)]
pub(crate) enum WorkPullingProducerControllerCommandKind<A>
where
  A: Clone + Send + Sync + 'static, {
  /// Initial message from the producer actor.
  Start { producer: TypedActorRef<WorkPullingProducerControllerRequestNext<A>> },
  /// A message from the producer (via `send_next_to`).
  Msg { message: A },
  /// Query for current worker statistics.
  GetWorkerStats { reply_to: TypedActorRef<WorkerStats> },
  /// Updated listing from the Receptionist (internal).
  WorkerListing { listing: Listing },
  /// Internal: a per-worker ProducerController has demand (sent RequestNext).
  InternalDemand { request: ProducerControllerRequestNext<A> },
  /// Loaded durable queue state owned by this controller.
  DurableQueueLoaded { state: DurableProducerQueueState<A> },
  /// A durable queue write completed for a message that can now be delivered.
  DurableQueueMessageStored { ack: StoreMessageSentAck },
  /// Internal timer: durable queue load timed out.
  DurableQueueLoadTimedOut { attempt: u32 },
  /// Internal timer: durable queue store timed out.
  DurableQueueStoreTimedOut { seq_nr: u64, attempt: u32 },
  /// Internal timer: a worker did not acknowledge a delivered message in time.
  WorkerDeliveryTimedOut { worker_key: u64, worker_local_seq_nr: u64 },
  /// Replay a previously persisted unconfirmed message.
  ReplayStoredMessage { sent: MessageSent<A> },
}

impl<A> WorkPullingProducerControllerCommand<A>
where
  A: Clone + Send + Sync + 'static,
{
  /// Creates a `Start` command.
  pub(crate) const fn start(producer: TypedActorRef<WorkPullingProducerControllerRequestNext<A>>) -> Self {
    Self(WorkPullingProducerControllerCommandKind::Start { producer })
  }

  /// Creates a `Msg` command (internal, from producer via send_next_to adapter).
  pub(crate) const fn msg(message: A) -> Self {
    Self(WorkPullingProducerControllerCommandKind::Msg { message })
  }

  /// Creates a `GetWorkerStats` command.
  pub(crate) const fn get_worker_stats(reply_to: TypedActorRef<WorkerStats>) -> Self {
    Self(WorkPullingProducerControllerCommandKind::GetWorkerStats { reply_to })
  }

  /// Creates a `WorkerListing` command (internal, from Receptionist subscription).
  pub(crate) const fn worker_listing(listing: Listing) -> Self {
    Self(WorkPullingProducerControllerCommandKind::WorkerListing { listing })
  }

  /// Creates an `InternalDemand` command (internal, from per-worker ProducerController).
  pub(crate) const fn internal_demand(request: ProducerControllerRequestNext<A>) -> Self {
    Self(WorkPullingProducerControllerCommandKind::InternalDemand { request })
  }

  /// Creates a `DurableQueueLoaded` command (internal).
  pub(crate) const fn durable_queue_loaded(state: DurableProducerQueueState<A>) -> Self {
    Self(WorkPullingProducerControllerCommandKind::DurableQueueLoaded { state })
  }

  /// Creates a `DurableQueueMessageStored` command (internal).
  pub(crate) const fn durable_queue_message_stored(ack: StoreMessageSentAck) -> Self {
    Self(WorkPullingProducerControllerCommandKind::DurableQueueMessageStored { ack })
  }

  /// Creates a `DurableQueueLoadTimedOut` command (internal timer).
  pub(crate) const fn durable_queue_load_timed_out(attempt: u32) -> Self {
    Self(WorkPullingProducerControllerCommandKind::DurableQueueLoadTimedOut { attempt })
  }

  /// Creates a `DurableQueueStoreTimedOut` command (internal timer).
  pub(crate) const fn durable_queue_store_timed_out(seq_nr: u64, attempt: u32) -> Self {
    Self(WorkPullingProducerControllerCommandKind::DurableQueueStoreTimedOut { seq_nr, attempt })
  }

  /// Creates a `WorkerDeliveryTimedOut` command (internal timer).
  pub(crate) const fn worker_delivery_timed_out(worker_key: u64, worker_local_seq_nr: u64) -> Self {
    Self(WorkPullingProducerControllerCommandKind::WorkerDeliveryTimedOut { worker_key, worker_local_seq_nr })
  }

  /// Creates a `ReplayStoredMessage` command (internal).
  pub(crate) const fn replay_stored_message(sent: MessageSent<A>) -> Self {
    Self(WorkPullingProducerControllerCommandKind::ReplayStoredMessage { sent })
  }

  /// Returns a reference to the command kind.
  pub(crate) const fn kind(&self) -> &WorkPullingProducerControllerCommandKind<A> {
    &self.0
  }
}
