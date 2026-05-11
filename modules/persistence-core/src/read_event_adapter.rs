//! Read-side adapter abstraction for replay payload conversion.

#[cfg(test)]
#[path = "read_event_adapter_test.rs"]
mod tests;

use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::event_seq::EventSeq;

/// Converts journal payloads back into one or many domain events.
pub trait ReadEventAdapter: Send + Sync + 'static {
  /// Converts a journal payload and manifest into domain events.
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq;
}
