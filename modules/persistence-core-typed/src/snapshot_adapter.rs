//! Typed snapshot adapter contract.

#[cfg(test)]
#[path = "snapshot_adapter_test.rs"]
mod tests;

use alloc::string::String;
use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

/// Converts typed state snapshots to and from snapshot payloads.
///
/// Runtime wiring for snapshot adapters is intentionally left to a follow-up change.
pub trait SnapshotAdapter<S>: Send + Sync + 'static {
  /// Returns the manifest associated with the state snapshot.
  fn manifest(&self, state: &S) -> String;

  /// Converts typed state into a snapshot payload representation.
  fn to_snapshot(&self, state: S) -> ArcShared<dyn Any + Send + Sync>;

  /// Converts a snapshot payload and manifest back into typed state.
  fn adapt_from_snapshot(&self, snapshot: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> Option<S>;
}
