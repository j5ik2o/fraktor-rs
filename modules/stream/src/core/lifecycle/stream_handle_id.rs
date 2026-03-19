use portable_atomic::{AtomicU64, Ordering};

/// Identifier for stream handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamHandleId(u64);

static STREAM_HANDLE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl StreamHandleId {
  /// Allocates a new handle identifier.
  #[must_use]
  pub fn next() -> Self {
    Self(STREAM_HANDLE_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
  }

  /// Returns the raw identifier.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.0
  }
}
