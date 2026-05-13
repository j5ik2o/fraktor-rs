//! Shared wrapper for Embassy executor.

use fraktor_utils_core_rs::sync::{ArcShared, ExclusiveCell, SharedAccess};

use super::embassy_executor::EmbassyExecutor;

/// Shared wrapper for [`EmbassyExecutor`].
///
/// Interior mutability is confined to this type; executor logic remains in
/// [`EmbassyExecutor`].
pub(crate) struct EmbassyExecutorShared<const N: usize> {
  inner: ArcShared<ExclusiveCell<EmbassyExecutor<N>>>,
}

impl<const N: usize> EmbassyExecutorShared<N> {
  /// Creates a new CAS-backed shared wrapper around the provided Embassy executor.
  #[must_use]
  pub(crate) fn new(executor: EmbassyExecutor<N>) -> Self {
    Self { inner: ArcShared::new(ExclusiveCell::new(executor)) }
  }
}

impl<const N: usize> Clone for EmbassyExecutorShared<N> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<const N: usize> SharedAccess<EmbassyExecutor<N>> for EmbassyExecutorShared<N> {
  fn with_read<R>(&self, f: impl FnOnce(&EmbassyExecutor<N>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EmbassyExecutor<N>) -> R) -> R {
    self.inner.with_write(f)
  }
}
