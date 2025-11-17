//! UID-agnostic equality helpers for actor paths.

use core::hash::{BuildHasher, Hash, Hasher};

use hashbrown::hash_map::DefaultHashBuilder;

use super::ActorPath;

/// Helper that compares and hashes actor paths while ignoring UIDs.
pub struct ActorPathComparator;

impl ActorPathComparator {
  /// Returns equality without considering the UID component.
  #[must_use]
  pub fn eq(lhs: &ActorPath, rhs: &ActorPath) -> bool {
    lhs.parts() == rhs.parts() && lhs.segments() == rhs.segments()
  }

  /// Computes a hash value that excludes the UID suffix.
  #[must_use]
  pub fn hash(path: &ActorPath) -> u64 {
    let mut hasher = DefaultHashBuilder::default().build_hasher();
    path.parts().hash(&mut hasher);
    path.segments().hash(&mut hasher);
    hasher.finish()
  }
}
