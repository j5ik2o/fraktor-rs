use core::ops::{Deref, DerefMut};
use std::sync::MutexGuard;

/// Guard returned by [`StdSyncMutex`](crate::StdSyncMutex).
pub struct StdSyncMutexGuard<'a, T> {
  pub(crate) guard: MutexGuard<'a, T>,
}

impl<'a, T> Deref for StdSyncMutexGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

impl<'a, T> DerefMut for StdSyncMutexGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.guard
  }
}
