//! Shared wrapper for actor cell registry.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::cells::Cells;

/// Shared wrapper for [`Cells`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct CellsShared {
  inner: ArcShared<RuntimeMutex<Cells>>,
}
#[allow(dead_code)]
impl CellsShared {
  /// Creates a new shared wrapper around the provided registry.
  #[must_use]
  pub(crate) fn new(cells: Cells) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(cells)) }
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
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Cells) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
