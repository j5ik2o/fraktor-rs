use alloc::collections::VecDeque;

use crate::collections::{QueueError, QueueSize};

/// Buffer implementation for multi-producer, single-consumer (MPSC) queues.
///
/// This buffer is used to exchange messages between multiple senders and a single receiver.
/// It supports both bounded and unbounded capacity options and manages closed state.
///
/// # Features
///
/// - Supports both bounded and unbounded modes
/// - Closed state management
/// - Thread-safe (when used with appropriate locking mechanisms)
///
/// # Examples
///
/// ```
/// use cellactor_utils_core_rs::MpscBuffer;
///
/// let mut buffer = MpscBuffer::new(Some(10));
/// assert!(buffer.offer(42).is_ok());
/// assert_eq!(buffer.poll().unwrap(), Some(42));
/// ```
#[derive(Debug, Clone)]
pub struct MpscBuffer<T> {
  buffer:   VecDeque<T>,
  capacity: Option<usize>,
  closed:   bool,
}

impl<T> MpscBuffer<T> {
  /// Creates a new `MpscBuffer`.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The buffer capacity. `None` for unlimited.
  ///
  /// # Returns
  ///
  /// A new `MpscBuffer` instance.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// // Bounded buffer with capacity 10
  /// let bounded = MpscBuffer::<i32>::new(Some(10));
  ///
  /// // Unbounded buffer
  /// let unbounded = MpscBuffer::<i32>::new(None);
  /// ```
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn new(capacity: Option<usize>) -> Self {
    Self { buffer: VecDeque::new(), capacity, closed: false }
  }

  /// Returns the current number of elements in the buffer.
  ///
  /// # Returns
  ///
  /// A `QueueSize` representing the current number of elements.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  /// assert_eq!(buffer.len().to_usize(), 2);
  /// ```
  #[must_use]
  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.buffer.len())
  }

  /// Returns the capacity of the buffer.
  ///
  /// # Returns
  ///
  /// A `QueueSize` representing the buffer capacity. Returns `QueueSize::limitless()` if unlimited.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let bounded = MpscBuffer::<i32>::new(Some(10));
  /// assert_eq!(bounded.capacity().to_usize(), 10);
  ///
  /// let unbounded = MpscBuffer::<i32>::new(None);
  /// assert!(unbounded.capacity().is_limitless());
  /// ```
  #[must_use]
  pub const fn capacity(&self) -> QueueSize {
    match self.capacity {
      | Some(limit) => QueueSize::limited(limit),
      | None => QueueSize::limitless(),
    }
  }

  /// Sets the capacity of the buffer.
  ///
  /// If the new capacity is smaller than the current buffer size, the buffer will be truncated.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The new capacity. `None` for unlimited.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  /// buffer.offer(3).unwrap();
  ///
  /// // Reduce capacity to 2 (third element will be removed)
  /// buffer.set_capacity(Some(2));
  /// assert_eq!(buffer.len().to_usize(), 2);
  /// ```
  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity
      && self.buffer.len() > limit
    {
      self.buffer.truncate(limit);
    }
  }

  /// Adds an element to the buffer.
  ///
  /// Returns an error if the buffer is closed or at capacity.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add.
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added.
  /// * `Err(QueueError::Closed(element))` - Buffer is closed.
  /// * `Err(QueueError::Full(element))` - Buffer is at capacity.
  ///
  /// # Errors
  ///
  /// * `QueueError::Closed(element)` - Buffer is closed.
  /// * `QueueError::Full(element)` - Buffer is at capacity.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::{MpscBuffer, QueueError};
  ///
  /// let mut buffer = MpscBuffer::new(Some(2));
  /// assert!(buffer.offer(1).is_ok());
  /// assert!(buffer.offer(2).is_ok());
  ///
  /// // Capacity exceeded
  /// assert!(matches!(buffer.offer(3), Err(QueueError::Full(_))));
  /// ```
  pub fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(element));
    }
    if matches!(self.capacity, Some(limit) if self.buffer.len() >= limit) {
      return Err(QueueError::Full(element));
    }
    self.buffer.push_back(element);
    Ok(())
  }

  /// Retrieves an element from the buffer.
  ///
  /// Returns `Disconnected` error if the buffer is empty and closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved.
  /// * `Ok(None)` - Buffer is empty (not yet closed).
  /// * `Err(QueueError::Disconnected)` - Buffer is empty and closed.
  ///
  /// # Errors
  ///
  /// * `QueueError::Disconnected` - Buffer is empty and closed.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::{MpscBuffer, QueueError};
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(42).unwrap();
  ///
  /// assert_eq!(buffer.poll().unwrap(), Some(42));
  /// assert_eq!(buffer.poll().unwrap(), None);
  ///
  /// buffer.mark_closed();
  /// assert!(matches!(buffer.poll(), Err(QueueError::Disconnected)));
  /// ```
  pub fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    if let Some(item) = self.buffer.pop_front() {
      return Ok(Some(item));
    }
    if self.closed { Err(QueueError::Disconnected) } else { Ok(None) }
  }

  /// Cleans up the buffer and marks it as closed.
  ///
  /// Removes all elements and marks the buffer as closed.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  /// buffer.offer(2).unwrap();
  ///
  /// buffer.clean_up();
  /// assert!(buffer.is_closed());
  /// assert_eq!(buffer.len().to_usize(), 0);
  /// ```
  pub fn clean_up(&mut self) {
    self.buffer.clear();
    self.closed = true;
  }

  /// Returns whether the buffer is closed.
  ///
  /// # Returns
  ///
  /// `true` if the buffer is closed, `false` otherwise.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let mut buffer = MpscBuffer::<i32>::new(Some(10));
  /// assert!(!buffer.is_closed());
  ///
  /// buffer.mark_closed();
  /// assert!(buffer.is_closed());
  /// ```
  #[must_use]
  pub const fn is_closed(&self) -> bool {
    self.closed
  }

  /// Marks the buffer as closed.
  ///
  /// Existing elements are not removed. Only adding new elements is forbidden.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::{MpscBuffer, QueueError};
  ///
  /// let mut buffer = MpscBuffer::new(Some(10));
  /// buffer.offer(1).unwrap();
  ///
  /// buffer.mark_closed();
  /// assert!(buffer.is_closed());
  ///
  /// // Cannot add elements after closing
  /// assert!(matches!(buffer.offer(2), Err(QueueError::Closed(_))));
  ///
  /// // But can still retrieve existing elements
  /// assert_eq!(buffer.poll().unwrap(), Some(1));
  /// ```
  pub const fn mark_closed(&mut self) {
    self.closed = true;
  }
}

impl<T> Default for MpscBuffer<T> {
  /// Creates a default `MpscBuffer`.
  ///
  /// Returns an unbounded buffer.
  ///
  /// # Examples
  ///
  /// ```
  /// use cellactor_utils_core_rs::MpscBuffer;
  ///
  /// let buffer: MpscBuffer<i32> = Default::default();
  /// assert!(buffer.capacity().is_limitless());
  /// ```
  fn default() -> Self {
    Self::new(None)
  }
}
