//! `AShared` wrapper around the pure `Association` state machine.

use core::fmt::{Debug, Formatter, Result as FmtResult};

use fraktor_remote_core_rs::association::Association;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

/// Thread-safe shared handle for an [`Association`].
///
/// `AssociationShared` follows the project-wide `AShared` pattern (see
/// `.agents/rules/rust/immutability-policy.md`): the pure `&mut self` API of
/// the inner `Association` is preserved by routing every access through a
/// `SpinSyncMutex` wrapped in an `ArcShared`. The wrapper itself only
/// exposes a `with_write` accessor — there is no read-only path because
/// every meaningful `Association` operation is a state transition.
pub struct AssociationShared {
  inner: ArcShared<SpinSyncMutex<Association>>,
}

impl AssociationShared {
  /// Wraps an [`Association`] into a shared handle.
  #[must_use]
  pub fn new(association: Association) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(association)) }
  }

  /// Borrows the inner `Association` mutably for the duration of `f`.
  ///
  /// The lock is held only for the duration of the closure body and is
  /// released as soon as the closure returns. Long-running operations should
  /// extract the data they need and drop the lock before doing additional
  /// work.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut Association) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl Clone for AssociationShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl Debug for AssociationShared {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("AssociationShared").finish_non_exhaustive()
  }
}
