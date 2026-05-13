//! Read-only access to the cluster block list.
//!
//! Moved here from the legacy `fraktor-remote-rs::core::block_list_provider`
//! during the `remote-redesign` change. Block-list ownership belongs to the
//! cluster layer, not the remote transport — the redesign places the trait
//! next to its only consumer (`fraktor-cluster-core-rs`).

use alloc::{string::String, vec::Vec};

/// Provides read access to blocked member identifiers.
pub trait BlockListProvider: Send + Sync {
  /// Returns the identifiers of members that are currently blocked.
  fn blocked_members(&self) -> Vec<String>;
}
