#[cfg(feature = "alloc")]
use core::cell::RefCell;

use crate::{
  MpscBuffer,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

/// Ring buffer-based storage abstraction trait
///
/// Provides storage abstraction shared by
/// [`crate::collections::queue_old::mpsc::RingBufferBackend`] implementations. This trait offers an
/// interface for uniformly handling read and write access to MPSC buffers.
///
/// # Type Parameters
///
/// * `T` - Type of elements stored in the buffer
pub trait RingBufferStorage<T> {
  /// Executes a closure using an immutable reference to the MPSC buffer
  ///
  /// # Arguments
  ///
  /// * `f` - Closure receiving an immutable reference to the MPSC buffer
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;

  /// Executes a closure using a mutable reference to the MPSC buffer
  ///
  /// # Arguments
  ///
  /// * `f` - Closure receiving a mutable reference to the MPSC buffer
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

#[cfg(feature = "alloc")]
impl<T> RingBufferStorage<T> for RefCell<MpscBuffer<T>> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
    let guard = self.borrow();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut guard)
  }
}

impl<T> RingBufferStorage<T> for SpinSyncMutex<MpscBuffer<T>> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
    let guard = self.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
    let mut guard = self.lock();
    f(&mut guard)
  }
}

impl<T> RingBufferStorage<T> for ArcShared<SpinSyncMutex<MpscBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
    (**self).with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
    (**self).with_write(f)
  }
}
