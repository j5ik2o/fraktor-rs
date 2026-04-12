//! Factory contract for [`ActorShared`](super::ActorShared).

use alloc::boxed::Box;

use super::{Actor, ActorShared};

/// Materializes [`ActorShared`] instances.
pub trait ActorSharedFactory: Send + Sync {
  /// Creates a shared actor wrapper.
  fn create(&self, actor: Box<dyn Actor + Send>) -> ActorShared;
}
