use alloc::{boxed::Box, vec::Vec};
use core::{fmt::Debug, mem::MaybeUninit};

use crate::{
    QueueError, QueueSize,
    collections::queue_old::traits::{QueueBase, QueueReader, QueueWriter},
};

#[cfg(test)]
mod tests;

/// Default capacity for ring buffers
///
/// Used as the initial capacity when ring buffers are created.
pub const DEFAULT_CAPACITY: usize = 32;

/// Ring buffer implementation
///
/// A circular buffer that operates as a FIFO queue.
/// Behavior when capacity is reached is controlled by the `dynamic` flag:
/// - `dynamic = true`: Capacity is automatically doubled
/// - `dynamic = false`: Adding new elements returns a `QueueError::Full` error
///
/// # Type Parameters
///
/// * `T` - Type of elements stored in the buffer
#[derive(Debug)]
pub struct RingBuffer<T> {
  /// Internal buffer (may contain uninitialized memory)
  buf:     Box<[MaybeUninit<T>]>,
  /// Head index indicating read position
  head:    usize,
  /// Tail index indicating write position
  tail:    usize,
  /// Current number of elements in the buffer
  len:     usize,
  /// Whether dynamic capacity expansion is enabled
  dynamic: bool,
}

impl<T> RingBuffer<T> {
  /// Creates a new ring buffer with the specified capacity
  ///
  /// Dynamic expansion is enabled by default (`dynamic = true`).
  ///
  /// # Parameters
  ///
  /// * `capacity` - Initial capacity (must be greater than 0)
  ///
  /// # Panics
  ///
  /// * Panics if `capacity` is 0
  ///
  /// # Examples
  ///
  /// ```
  /// # use cellactor_utils_core_rs::RingBuffer;
  /// let buffer = RingBuffer::<i32>::new(10);
  /// ```
  #[must_use]
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "capacity must be > 0");
    let buf = Self::alloc_buffer(capacity);
    Self { buf, head: 0, tail: 0, len: 0, dynamic: true }
  }

  /// Sets dynamic expansion enabled/disabled and returns the ring buffer (builder pattern)
  ///
  /// # Parameters
  ///
  /// * `dynamic` - If `true`, automatically expands when capacity is reached
  ///
  /// # Examples
  ///
  /// ```
  /// # use cellactor_utils_core_rs::RingBuffer;
  /// let buffer = RingBuffer::<i32>::new(10).with_dynamic(false);
  /// ```
  #[must_use]
  pub const fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.dynamic = dynamic;
    self
  }

  /// Sets dynamic expansion enabled/disabled
  ///
  /// # Parameters
  ///
  /// * `dynamic` - If `true`, automatically expands when capacity is reached
  pub const fn set_dynamic(&mut self, dynamic: bool) {
    self.dynamic = dynamic;
  }

  /// Allocates a new buffer with the specified capacity
  ///
  /// # Parameters
  ///
  /// * `capacity` - Capacity to allocate
  ///
  /// # Returns
  ///
  /// Boxed slice containing uninitialized memory
  fn alloc_buffer(capacity: usize) -> Box<[MaybeUninit<T>]> {
    let mut vec = Vec::with_capacity(capacity);
    vec.resize_with(capacity, MaybeUninit::uninit);
    vec.into_boxed_slice()
  }

  /// Doubles the buffer capacity
  ///
  /// Removes all existing elements and reinserts them into a new buffer.
  /// Existing elements are preserved during this operation.
  ///
  /// # Panics
  ///
  /// Panics if insufficient capacity occurs during reinsertion
  /// (should normally not occur)
  fn grow(&mut self) {
    let new_cap = self.buf.len().saturating_mul(2).max(1);
    let mut items = Vec::with_capacity(self.len);
    while let Ok(Some(item)) = self.poll_mut() {
      items.push(item);
    }

    self.buf = Self::alloc_buffer(new_cap);
    self.head = 0;
    self.tail = 0;
    self.len = 0;

    for item in items {
      // reinsert without triggering another grow
      if let Err(QueueError::Full(_)) = self.offer_mut(item) {
        panic!("ring buffer grow failed to reserve capacity");
      }
    }
  }
}

impl<T> QueueBase<T> for RingBuffer<T> {
  /// Returns the current number of elements in the buffer
  ///
  /// # Returns
  ///
  /// `QueueSize::limited` representing element count
  fn len(&self) -> QueueSize {
    QueueSize::limited(self.len)
  }

  /// Returns the buffer capacity
  ///
  /// # Returns
  ///
  /// * If `dynamic = true`: `QueueSize::limitless()` (unlimited)
  /// * If `dynamic = false`: `QueueSize::limited(capacity)` (limited)
  fn capacity(&self) -> QueueSize {
    if self.dynamic { QueueSize::limitless() } else { QueueSize::limited(self.buf.len()) }
  }
}

impl<T> QueueWriter<T> for RingBuffer<T> {
  /// Adds an element to the buffer
  ///
  /// When buffer is full:
  /// * `dynamic = true`: Doubles capacity before adding
  /// * `dynamic = false`: Returns `QueueError::Full`
  ///
  /// # Parameters
  ///
  /// * `item` - Element to add
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If addition succeeded
  /// * `Err(QueueError::Full(item))` - If buffer is full and dynamic expansion is disabled
  ///
  /// # Examples
  ///
  /// ```
  /// # use cellactor_utils_core_rs::{QueueWriter, RingBuffer};
  /// let mut buffer = RingBuffer::new(2).with_dynamic(false);
  /// buffer.offer_mut(1).unwrap();
  /// buffer.offer_mut(2).unwrap();
  /// assert!(buffer.offer_mut(3).is_err()); // Error because full
  /// ```
  fn offer_mut(&mut self, item: T) -> Result<(), QueueError<T>> {
    if self.len == self.buf.len() {
      if self.dynamic {
        self.grow();
      } else {
        return Err(QueueError::Full(item));
      }
    }

    self.buf[self.tail] = MaybeUninit::new(item);
    self.tail = (self.tail + 1) % self.buf.len();
    self.len += 1;
    Ok(())
  }
}

impl<T> QueueReader<T> for RingBuffer<T> {
  /// Removes an element from the buffer (FIFO order)
  ///
  /// # Returns
  ///
  /// * `Ok(Some(item))` - If element was removed
  /// * `Ok(None)` - If buffer is empty
  ///
  /// # Examples
  ///
  /// ```
  /// # use cellactor_utils_core_rs::{QueueWriter, QueueReader, RingBuffer};
  /// let mut buffer = RingBuffer::new(10);
  /// buffer.offer_mut(1).unwrap();
  /// buffer.offer_mut(2).unwrap();
  /// assert_eq!(buffer.poll_mut().unwrap(), Some(1));
  /// assert_eq!(buffer.poll_mut().unwrap(), Some(2));
  /// assert_eq!(buffer.poll_mut().unwrap(), None);
  /// ```
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    if self.len == 0 {
      return Ok(None);
    }

    let value = unsafe { self.buf[self.head].assume_init_read() };
    self.head = (self.head + 1) % self.buf.len();
    self.len -= 1;
    Ok(Some(value))
  }

  /// Discards all elements in the buffer
  ///
  /// Removes and drops all elements in order.
  /// After this operation, the buffer is empty.
  fn clean_up_mut(&mut self) {
    while self.len > 0 {
      let _ = self.poll_mut();
    }
  }
}

impl<T> Default for RingBuffer<T> {
  /// Creates a default ring buffer
  ///
  /// Creates a buffer with `DEFAULT_CAPACITY` (32) capacity and dynamic expansion enabled.
  ///
  /// # Examples
  ///
  /// ```
  /// # use cellactor_utils_core_rs::RingBuffer;
  /// let buffer = RingBuffer::<i32>::default();
  /// ```
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}
