//! No-op event adapter implementation.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{event_seq::EventSeq, read_event_adapter::ReadEventAdapter, write_event_adapter::WriteEventAdapter};

/// Default adapter that keeps event payloads unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct IdentityEventAdapter;

impl IdentityEventAdapter {
  /// Creates a new identity adapter.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl WriteEventAdapter for IdentityEventAdapter {
  fn manifest(&self, _event: &(dyn Any + Send + Sync)) -> String {
    String::new()
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    event
  }
}

impl ReadEventAdapter for IdentityEventAdapter {
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, _manifest: &str) -> EventSeq {
    EventSeq::single(event)
  }
}
