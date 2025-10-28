use alloc::boxed::Box;

use async_trait::async_trait;

use super::{PushOutcome, StackError};
use crate::v2::collections::wait::WaitHandle;

/// Async-compatible backend trait for stack operations.
#[async_trait(?Send)]
pub trait AsyncStackBackend<T> {
  /// Pushes an element onto the stack.
  async fn push(&mut self, item: T) -> Result<PushOutcome, StackError>;

  /// Pops the top element from the stack.
  async fn pop(&mut self) -> Result<T, StackError>;

  /// Returns a reference to the top element without removing it.
  fn peek(&self) -> Option<&T>;

  /// Transitions the backend into the closed state.
  async fn close(&mut self) -> Result<(), StackError>;

  /// Returns the number of stored elements.
  fn len(&self) -> usize;

  /// Returns the storage capacity.
  fn capacity(&self) -> usize;

  /// Indicates whether the stack is empty.
  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the stack is full.
  fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Optionally registers a waiter when the stack is full and pushes should block.
  fn prepare_push_wait(&mut self) -> Option<WaitHandle<StackError>> {
    let _ = self;
    None
  }

  /// Optionally registers a waiter when the stack is empty and pops should block.
  fn prepare_pop_wait(&mut self) -> Option<WaitHandle<StackError>> {
    let _ = self;
    None
  }

  /// Indicates whether the backend has been closed.
  fn is_closed(&self) -> bool {
    false
  }
}
