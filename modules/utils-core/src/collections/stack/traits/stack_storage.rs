#[cfg(feature = "alloc")]
use core::cell::RefCell;

use crate::collections::stack::buffer::StackBuffer;

/// Abstraction for storage used by stack backends.
pub trait StackStorage<T> {
  /// Executes a closure with read-only access.
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R;

  /// Executes a closure with writable access.
  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R;
}

#[cfg(feature = "alloc")]
impl<T> StackStorage<T> for RefCell<StackBuffer<T>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    f(&self.borrow())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    f(&mut self.borrow_mut())
  }
}
