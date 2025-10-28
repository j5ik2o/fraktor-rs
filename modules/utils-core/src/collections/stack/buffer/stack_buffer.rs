use alloc::vec::Vec;

use super::stack_error::StackError;
use crate::collections::QueueSize;

/// Owned stack buffer that stores values in LIFO (Last-In-First-Out) order.
#[derive(Debug, Clone)]
pub struct StackBuffer<T> {
  items:    Vec<T>,
  capacity: Option<usize>,
}

impl<T> StackBuffer<T> {
  /// Creates a new empty `StackBuffer` without capacity limit.
  #[must_use]
  pub const fn new() -> Self {
    Self { items: Vec::new(), capacity: None }
  }

  /// Creates a new `StackBuffer` with the specified capacity limit.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self { items: Vec::with_capacity(capacity), capacity: Some(capacity) }
  }

  /// Gets the stack's capacity limit.
  #[must_use]
  pub const fn capacity(&self) -> QueueSize {
    match self.capacity {
      | Some(limit) => QueueSize::limited(limit),
      | None => QueueSize::limitless(),
    }
  }

  /// Sets the stack's capacity limit.
  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity {
      if self.items.len() > limit {
        self.items.truncate(limit);
      }
    }
  }

  /// Gets the current number of elements in the stack.
  #[must_use]
  pub const fn len(&self) -> QueueSize {
    QueueSize::limited(self.items.len())
  }

  /// Checks if the stack is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  /// Adds an element to the top of the stack.
  pub fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    if let Some(limit) = self.capacity {
      if self.items.len() >= limit {
        return Err(StackError::Full(value));
      }
    }
    self.items.push(value);
    Ok(())
  }

  /// Removes and returns the top element of the stack.
  pub fn pop(&mut self) -> Option<T> {
    self.items.pop()
  }

  /// Clears all elements from the stack.
  pub fn clear(&mut self) {
    self.items.clear();
  }

  /// Returns a reference to the top element of the stack.
  #[must_use]
  pub fn peek(&self) -> Option<&T> {
    self.items.last()
  }
}

impl<T> Default for StackBuffer<T> {
  fn default() -> Self {
    Self::new()
  }
}
