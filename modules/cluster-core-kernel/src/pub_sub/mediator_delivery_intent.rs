//! Delivery intent produced by mediator protocol semantics.

use alloc::vec::Vec;

use super::{MediatorDeliveryMode, MediatorPathKey, PubSubEnvelope, PubSubSubscriber};

/// Delivery intent produced by mediator selection without executing delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorDeliveryIntent {
  /// Deliver payload to selected targets.
  Deliver {
    /// Delivery mode that produced the target set.
    mode:    MediatorDeliveryMode,
    /// Selected subscriber targets.
    targets: Vec<PubSubSubscriber>,
    /// Serialized payload.
    payload: PubSubEnvelope,
  },
  /// Drop payload because no matching target exists.
  Dropped {
    /// Requested path key.
    path:    MediatorPathKey,
    /// Serialized payload.
    payload: PubSubEnvelope,
  },
  /// Emit dead-letter payload because no matching target exists.
  DeadLetter {
    /// Requested path key.
    path:    MediatorPathKey,
    /// Serialized payload.
    payload: PubSubEnvelope,
  },
}

impl MediatorDeliveryIntent {
  /// Returns selected targets when this is a delivery intent.
  #[must_use]
  pub fn targets(&self) -> &[PubSubSubscriber] {
    match self {
      | Self::Deliver { targets, .. } => targets,
      | Self::Dropped { .. } | Self::DeadLetter { .. } => &[],
    }
  }
}
