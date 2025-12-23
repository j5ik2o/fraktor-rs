use portable_atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

/// Identifier for stream ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(u64);

static PORT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl PortId {
  /// Allocates a new port identifier.
  #[must_use]
  pub fn next() -> Self {
    Self(PORT_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
  }

  /// Returns the raw identifier value.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.0
  }
}
