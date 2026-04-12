//! Factory contract for [`ReceiveTimeoutStateShared`](super::ReceiveTimeoutStateShared).

use super::{ReceiveTimeoutState, ReceiveTimeoutStateShared};

/// Materializes [`ReceiveTimeoutStateShared`] instances.
pub trait ReceiveTimeoutStateSharedFactory: Send + Sync {
  /// Creates a shared receive-timeout runtime state wrapper.
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared;
}
