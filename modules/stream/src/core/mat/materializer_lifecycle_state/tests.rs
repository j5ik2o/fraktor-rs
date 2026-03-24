use super::MaterializerLifecycleState;

// ---------------------------------------------------------------------------
// Construction / variant identity
// ---------------------------------------------------------------------------

#[test]
fn idle_variant_is_distinct_from_running_and_stopped() {
  // Given: three lifecycle states
  let idle = MaterializerLifecycleState::Idle;
  let running = MaterializerLifecycleState::Running;
  let stopped = MaterializerLifecycleState::Stopped;

  // Then: each variant is distinct
  assert_ne!(idle, running);
  assert_ne!(idle, stopped);
  assert_ne!(running, stopped);
}

#[test]
fn same_variants_are_equal() {
  // Given: two instances of the same variant
  let a = MaterializerLifecycleState::Running;
  let b = MaterializerLifecycleState::Running;

  // Then: they are equal
  assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_produces_equal_value() {
  // Given: a lifecycle state
  let original = MaterializerLifecycleState::Running;

  // When: cloned
  let cloned = original;

  // Then: the clone equals the original (Copy)
  assert_eq!(original, cloned);
}

#[test]
fn debug_format_contains_variant_name() {
  // Given: each variant
  // Then: Debug output contains the variant name
  let idle = format!("{:?}", MaterializerLifecycleState::Idle);
  let running = format!("{:?}", MaterializerLifecycleState::Running);
  let stopped = format!("{:?}", MaterializerLifecycleState::Stopped);

  assert!(idle.contains("Idle"));
  assert!(running.contains("Running"));
  assert!(stopped.contains("Stopped"));
}

// ---------------------------------------------------------------------------
// Hash consistency (equal values must produce equal hashes)
// ---------------------------------------------------------------------------

#[test]
fn equal_variants_produce_equal_hashes() {
  use core::hash::Hash;

  // Given: two instances of the same variant
  let a = MaterializerLifecycleState::Stopped;
  let b = MaterializerLifecycleState::Stopped;

  // When: hashed
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

  // Then: hashes are equal
  assert_eq!(hash_a, hash_b);
}

/// Minimal hasher that collects the first u64 written.
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
