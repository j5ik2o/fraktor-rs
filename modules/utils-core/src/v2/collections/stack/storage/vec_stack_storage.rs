use alloc::{collections::TryReserveError, vec::Vec};

use super::StackStorage;

/// Contiguous stack storage backed by `alloc::vec::Vec`.
pub struct VecStackStorage<T> {
  data:  Vec<T>,
  limit: usize,
}

impl<T> VecStackStorage<T> {
  /// Creates a storage buffer with the provided capacity limit.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self { data: Vec::with_capacity(capacity), limit: capacity }
  }

  /// Returns the number of initialized elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.data.len()
  }

  /// Returns whether the storage currently holds no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }

  /// Returns the configured capacity limit.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.limit
  }

  /// Pushes an element onto the end of the storage without additional checks.
  pub fn push(&mut self, value: T) {
    debug_assert!(self.len() < self.limit);
    self.data.push(value);
  }

  /// Pops the last element from storage.
  pub fn pop(&mut self) -> Option<T> {
    self.data.pop()
  }

  /// Returns a reference to the last element if it exists.
  pub fn peek(&self) -> Option<&T> {
    self.data.last()
  }

  /// Attempts to grow the capacity limit to `new_capacity`.
  pub fn try_grow(&mut self, new_capacity: usize) -> Result<(), TryReserveError> {
    if new_capacity <= self.limit {
      return Ok(());
    }
    let additional = new_capacity - self.limit;
    self.data.try_reserve(additional)?;
    self.limit = new_capacity;
    Ok(())
  }
}

impl<T> StackStorage<T> for VecStackStorage<T> {
  fn capacity(&self) -> usize {
    self.limit
  }

  unsafe fn read_unchecked(&self, idx: usize) -> *const T {
    debug_assert!(idx < self.len());
    unsafe { self.data.as_ptr().add(idx) }
  }

  unsafe fn write_unchecked(&mut self, idx: usize, val: T) {
    if idx == self.len() {
      self.data.push(val);
    } else {
      debug_assert!(idx < self.len());
      if let Some(slot) = self.data.get_mut(idx) {
        *slot = val;
      }
    }
  }
}
