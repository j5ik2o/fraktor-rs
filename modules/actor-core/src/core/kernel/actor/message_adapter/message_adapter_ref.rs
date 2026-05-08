//! Opaque actor reference for message adapters.

use super::MessageAdapterLease;
use crate::core::kernel::actor::actor_ref::ActorRef;

/// Actor reference registered for message adapter delivery.
pub struct MessageAdapterRef {
  actor_ref: ActorRef,
  lease:     MessageAdapterLease,
}

impl MessageAdapterRef {
  #[must_use]
  pub(crate) const fn new(actor_ref: ActorRef, lease: MessageAdapterLease) -> Self {
    Self { actor_ref, lease }
  }

  /// Splits the adapter reference into its send target and release handle.
  #[must_use]
  pub fn into_parts(self) -> (ActorRef, MessageAdapterLease) {
    (self.actor_ref, self.lease)
  }
}
