use crate::core::snapshot::ConnectionState;

// ---------------------------------------------------------------------------
// Equality
// ---------------------------------------------------------------------------

#[test]
fn should_pull_equals_itself() {
  assert_eq!(ConnectionState::ShouldPull, ConnectionState::ShouldPull);
}

#[test]
fn should_push_equals_itself() {
  assert_eq!(ConnectionState::ShouldPush, ConnectionState::ShouldPush);
}

#[test]
fn closed_equals_itself() {
  assert_eq!(ConnectionState::Closed, ConnectionState::Closed);
}

#[test]
fn distinct_variants_are_not_equal() {
  // Given/Then: all three pairs of distinct variants are inequal
  assert_ne!(ConnectionState::ShouldPull, ConnectionState::ShouldPush);
  assert_ne!(ConnectionState::ShouldPull, ConnectionState::Closed);
  assert_ne!(ConnectionState::ShouldPush, ConnectionState::Closed);
}

// ---------------------------------------------------------------------------
// Copy / Clone semantics
// ---------------------------------------------------------------------------

#[test]
fn copy_semantics_preserve_variant() {
  // Given: a ConnectionState value
  let state = ConnectionState::ShouldPull;

  // When: bit-copied via Copy
  let copied = state;

  // Then: both retain the original variant (no move)
  assert_eq!(state, copied);
  assert_eq!(copied, ConnectionState::ShouldPull);
}

// Note: `Clone` derive for a `Copy` enum is exercised automatically by the
// `copy_semantics_preserve_variant` test above (Copy supersedes Clone for this
// use case — clippy's `clone_on_copy` lint forbids explicit `.clone()` on
// Copy types).

// ---------------------------------------------------------------------------
// Hash: equal values must hash equal
// ---------------------------------------------------------------------------

#[test]
fn equal_variants_produce_equal_hashes() {
  use core::hash::Hash;

  // Given: two equal ConnectionState values
  let a = ConnectionState::Closed;
  let b = ConnectionState::Closed;

  // When: hashed with the same hasher
  let hash_a = {
    let mut h = HashCollector::default();
    a.hash(&mut h);
    h.0
  };
  let hash_b = {
    let mut h = HashCollector::default();
    b.hash(&mut h);
    h.0
  };

  // Then: they produce the same hash
  assert_eq!(hash_a, hash_b);
}

/// Minimal hasher that collects a running digest of written bytes.
#[derive(Default)]
struct HashCollector(u64);

impl core::hash::Hasher for HashCollector {
  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, bytes: &[u8]) {
    for &b in bytes {
      self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(b));
    }
  }
}

// ---------------------------------------------------------------------------
// Debug formatting
// ---------------------------------------------------------------------------

#[test]
fn debug_format_shows_should_pull_variant_name() {
  let debug = alloc::format!("{:?}", ConnectionState::ShouldPull);
  assert_eq!(debug, "ShouldPull");
}

#[test]
fn debug_format_shows_should_push_variant_name() {
  let debug = alloc::format!("{:?}", ConnectionState::ShouldPush);
  assert_eq!(debug, "ShouldPush");
}

#[test]
fn debug_format_shows_closed_variant_name() {
  let debug = alloc::format!("{:?}", ConnectionState::Closed);
  assert_eq!(debug, "Closed");
}
