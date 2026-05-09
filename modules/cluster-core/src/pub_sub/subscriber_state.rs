//! Subscriber state classification.

use alloc::string::String;

/// Subscriber state tracked by the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriberState {
  /// Subscriber is active.
  Active,
  /// Subscriber is suspended with a reason.
  Suspended {
    /// Suspension reason.
    reason: String,
  },
}
