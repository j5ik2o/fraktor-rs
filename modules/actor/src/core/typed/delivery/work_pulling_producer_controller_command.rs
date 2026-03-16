//! Commands accepted by the work-pulling producer controller actor.

#[cfg(test)]
mod tests;

use crate::core::typed::{
  Listing,
  actor::TypedActorRef,
  delivery::{ProducerControllerRequestNext, WorkPullingProducerControllerRequestNext, WorkerStats},
};

/// Commands handled by
/// [`WorkPullingProducerController`](crate::core::typed::delivery::WorkPullingProducerController).
///
/// User code constructs commands through
/// [`WorkPullingProducerController`](crate::core::typed::delivery::WorkPullingProducerController)
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

  /// Returns a reference to the command kind.
  pub(crate) const fn kind(&self) -> &WorkPullingProducerControllerCommandKind<A> {
    &self.0
  }
}
