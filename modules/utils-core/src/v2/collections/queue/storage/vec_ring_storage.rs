use alloc::collections::{TryReserveError, VecDeque};
use core::ptr;

use super::QueueStorage;

/// Ring buffer storage backed by [`VecDeque`].
pub struct VecRingStorage<T> {
  buffer: VecDeque<T>,
  limit:  usize,
}

impl<T> VecRingStorage<T> {
  /// Creates a new storage with the specified capacity limit.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self { buffer: VecDeque::with_capacity(capacity), limit: capacity }
  }

  /// Returns the number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.buffer.len()
  }

  /// Indicates whether the storage is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  /// Indicates whether the storage is full.
  #[must_use]
  pub fn is_full(&self) -> bool {
    self.len() == self.limit
  }

  /// Pushes an element to the back of the buffer.
  pub fn push_back(&mut self, value: T) {
    debug_assert!(!self.is_full());
    self.buffer.push_back(value);
  }

  /// Pops an element from the front of the buffer.
  pub fn pop_front(&mut self) -> Option<T> {
    self.buffer.pop_front()
  }

  /// Pops an element from the back of the buffer.
  pub fn pop_back(&mut self) -> Option<T> {
    self.buffer.pop_back()
  }

  /// Attempts to grow the capacity limit to the provided value.
  pub fn try_grow(&mut self, new_capacity: usize) -> Result<(), TryReserveError> {
    if new_capacity <= self.limit {
      return Ok(());
    }
    let additional = new_capacity - self.limit;
    self.buffer.try_reserve(additional)?;
    self.limit = new_capacity;
    Ok(())
  }
}

impl<T> QueueStorage<T> for VecRingStorage<T> {
  fn capacity(&self) -> usize {
    self.limit
  }

  unsafe fn read_unchecked(&self, idx: usize) -> *const T {
    debug_assert!(idx < self.buffer.len());
    let (front, back) = self.buffer.as_slices();
    if idx < front.len() {
      unsafe { ptr::from_ref(front.get_unchecked(idx)) }
    } else {
      unsafe { ptr::from_ref(back.get_unchecked(idx - front.len())) }
    }
  }

  unsafe fn write_unchecked(&mut self, idx: usize, val: T) {
    if idx == self.buffer.len() {
      self.buffer.push_back(val);
      return;
    }
    debug_assert!(idx < self.buffer.len());
    let (front, back) = self.buffer.as_mut_slices();
    if idx < front.len() {
      unsafe {
        *front.get_unchecked_mut(idx) = val;
      }
    } else {
      unsafe {
        *back.get_unchecked_mut(idx - front.len()) = val;
      }
    }
  }
}
