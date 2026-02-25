//! Write-side adapter abstraction for journal payload conversion.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

/// Converts domain events before they are written to the journal.
pub trait WriteEventAdapter: Send + Sync + 'static {
  /// Returns the manifest (type hint) associated with the event.
  fn manifest(&self, event: &(dyn Any + Send + Sync)) -> String;

  /// Converts an event into a journal payload representation.
  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync>;
}
