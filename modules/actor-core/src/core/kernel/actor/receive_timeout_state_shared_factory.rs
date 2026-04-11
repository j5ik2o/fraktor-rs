//! Factory contract for [`ReceiveTimeoutStateShared`](super::ReceiveTimeoutStateShared).

use super::ReceiveTimeoutStateShared;

/// Materializes [`ReceiveTimeoutStateShared`] instances.
pub trait ReceiveTimeoutStateSharedFactory: Send + Sync {
  /// Creates a shared receive-timeout runtime state wrapper.
  fn create(&self) -> ReceiveTimeoutStateShared;
}
