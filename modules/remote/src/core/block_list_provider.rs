//! Exposes the current block list maintained by remoting transports.

use alloc::{string::String, vec::Vec};

/// Provides read access to blocked member identifiers.
pub trait BlockListProvider: Send + Sync {
  /// Returns the identifiers of members that are currently blocked.
  fn blocked_members(&self) -> Vec<String>;
}
