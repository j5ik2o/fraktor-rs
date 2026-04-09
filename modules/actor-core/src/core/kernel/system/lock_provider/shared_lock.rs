//! Shared lock primitive that abstracts over builtin and debug lock variants.

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::kernel::system::lock_provider::{DebugSpinLock, DebugSpinLockGuard};

pub(crate) enum SharedLockGuard<'a, T> {
  Builtin(spin::MutexGuard<'a, T>),
  Debug(DebugSpinLockGuard<'a, T>),
}

impl<T> core::ops::Deref for SharedLockGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

impl<T> core::ops::DerefMut for SharedLockGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

pub(crate) enum SharedLock<T> {
  Builtin(ArcShared<RuntimeMutex<T>>),
  Debug(ArcShared<DebugSpinLock<T>>),
}

impl<T> SharedLock<T> {
  pub(crate) fn builtin(value: T) -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(value)))
  }

  pub(crate) fn debug(value: T, label: &'static str) -> Self {
    Self::Debug(ArcShared::new(DebugSpinLock::new(value, label)))
  }

  pub(crate) fn lock(&self) -> SharedLockGuard<'_, T> {
    match self {
      | Self::Builtin(inner) => SharedLockGuard::Builtin(inner.lock()),
      | Self::Debug(inner) => SharedLockGuard::Debug(inner.lock()),
    }
  }
}

impl<T> Clone for SharedLock<T> {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug(inner) => Self::Debug(inner.clone()),
    }
  }
}
