//! Marker trait for typed behavior signals.

/// Marker trait for typed behavior lifecycle notifications.
///
/// Corresponds to Pekko's `Signal` contract.
pub trait Signal: Send + Sync + 'static {}
