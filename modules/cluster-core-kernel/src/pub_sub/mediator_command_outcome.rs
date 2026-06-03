//! Outcome produced by applying a mediator command.

use super::{MediatorAcknowledgement, MediatorDeliveryIntent, MediatorQueryResult, TopicRegistryVersion};

/// Observable result of a distributed pub-sub mediator command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorCommandOutcome {
  /// Command was accepted by a no-op mediator implementation.
  Noop,
  /// A registry mutation completed without a protocol acknowledgement.
  RegistryMutated {
    /// Local bucket version after the mutation.
    version: TopicRegistryVersion,
  },
  /// Subscribe or unsubscribe acknowledgement.
  Acknowledged(MediatorAcknowledgement),
  /// Delivery intent generated from registry state.
  Delivery(MediatorDeliveryIntent),
  /// Query result generated from registry state.
  Query(MediatorQueryResult),
}
