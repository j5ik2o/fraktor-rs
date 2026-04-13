//! Shared wrapper for actor cell registry.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use super::cells::Cells;

/// Shared wrapper for [`Cells`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct CellsShared {
  inner: SharedLock<Cells>,
}
#[allow(dead_code)]
impl CellsShared {
  /// Creates a new shared wrapper around the provided registry.
  #[must_use]
  pub(crate) fn new(cells: Cells) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(cells) }
  }
}

impl Default for CellsShared {
  fn default() -> Self {
    Self::new(Cells::default())
  }
}

impl Clone for CellsShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Cells> for CellsShared {
  fn with_read<R>(&self, f: impl FnOnce(&Cells) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Cells) -> R) -> R {
    self.inner.with_write(f)
  }
}
