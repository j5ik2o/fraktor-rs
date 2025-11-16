use alloc::vec::Vec;
use core::cmp;

use crate::collections::stack::{
  PushOutcome, StackError, StackOverflowPolicy, SyncStackBackend, backend::SyncStackBackendInternal,
};

/// Stack backend backed by a contiguous growable buffer.
pub struct VecStackBackend<T> {
  items:    Vec<T>,
  capacity: usize,
  policy:   StackOverflowPolicy,
  closed:   bool,
}

impl<T> VecStackBackend<T> {
  /// Creates a new backend with the specified initial capacity and overflow policy.
  #[must_use]
  pub fn with_capacity(capacity: usize, policy: StackOverflowPolicy) -> Self {
    Self { items: Vec::with_capacity(capacity), capacity, policy, closed: false }
  }

  fn ensure_capacity(&mut self, required: usize) -> Result<Option<usize>, StackError> {
    if required <= self.capacity {
      return Ok(None);
    }

    let current = self.capacity;
    let next = cmp::max(required, cmp::max(1, current.saturating_mul(2)));
    self.items.try_reserve(next.saturating_sub(self.items.len())).map_err(|_| StackError::AllocError)?;
    self.capacity = next;
    Ok(Some(next))
  }

  fn handle_grow_policy(&mut self, required: usize) -> Result<Option<usize>, StackError> {
    match self.policy {
      | StackOverflowPolicy::Block => Err(StackError::Full),
      | StackOverflowPolicy::Grow => self.ensure_capacity(required),
    }
  }
}

impl<T> SyncStackBackend<T> for VecStackBackend<T> {}

impl<T> SyncStackBackendInternal<T> for VecStackBackend<T> {
  fn push(&mut self, item: T) -> Result<PushOutcome, StackError> {
    if self.closed {
      return Err(StackError::Closed);
    }

    let len = self.items.len();
    if len == self.capacity {
      let grown_to = self.handle_grow_policy(len + 1)?;
      if let Some(capacity) = grown_to {
        self.items.push(item);
        return Ok(PushOutcome::GrewTo { capacity });
      }
    }

    self.items.push(item);
    Ok(PushOutcome::Pushed)
  }

  fn pop(&mut self) -> Result<T, StackError> {
    match self.items.pop() {
      | Some(item) => Ok(item),
      | None => {
        if self.closed {
          Err(StackError::Closed)
        } else {
          Err(StackError::Empty)
        }
      },
    }
  }

  fn peek(&self) -> Option<&T> {
    self.items.last()
  }

  fn len(&self) -> usize {
    self.items.len()
  }

  fn capacity(&self) -> usize {
    self.capacity
  }

  fn overflow_policy(&self) -> StackOverflowPolicy {
    self.policy
  }

  fn is_closed(&self) -> bool {
    self.closed
  }

  fn close(&mut self) {
    self.closed = true;
  }
}
