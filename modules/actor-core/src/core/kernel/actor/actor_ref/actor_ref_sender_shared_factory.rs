//! Factory contract for [`ActorRefSenderShared`](super::ActorRefSenderShared).

use alloc::boxed::Box;

use super::{ActorRefSender, ActorRefSenderShared};

/// Materializes [`ActorRefSenderShared`] instances.
pub trait ActorRefSenderSharedFactory: Send + Sync {
  /// Creates a shared actor-ref sender wrapper.
  fn create(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared;
}
