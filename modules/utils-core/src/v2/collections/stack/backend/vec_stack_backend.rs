use core::cmp;

use crate::v2::collections::stack::{PushOutcome, StackBackend, StackError, StackOverflowPolicy, VecStackStorage};

/// Stack backend backed by a contiguous growable buffer.
pub struct VecStackBackend<T> {
  storage: VecStackStorage<T>,
  policy:  StackOverflowPolicy,
  closed:  bool,
}

impl<T> VecStackBackend<T> {
  /// Creates a new backend with the provided storage configuration and overflow policy.
  #[must_use]
  pub const fn new_with_storage(storage: VecStackStorage<T>, policy: StackOverflowPolicy) -> Self {
    Self { storage, policy, closed: false }
  }

  fn ensure_capacity(&mut self, required: usize) -> Result<Option<usize>, StackError> {
    if required <= self.storage.capacity() {
      return Ok(None);
    }

    let current = self.storage.capacity();
    let next = cmp::max(required, cmp::max(1, current.saturating_mul(2)));
    self.storage.try_grow(next).map_err(|_| StackError::AllocError)?;
    Ok(Some(next))
  }

  fn handle_grow_policy(&mut self, required: usize) -> Result<Option<usize>, StackError> {
    match self.policy {
      | StackOverflowPolicy::Block => Err(StackError::Full),
      | StackOverflowPolicy::Grow => self.ensure_capacity(required),
    }
  }
}

impl<T> StackBackend<T> for VecStackBackend<T> {
  type Storage = VecStackStorage<T>;

  fn new(storage: Self::Storage, policy: StackOverflowPolicy) -> Self {
    VecStackBackend::new_with_storage(storage, policy)
  }

  fn push(&mut self, item: T) -> Result<PushOutcome, StackError> {
    if self.closed {
      return Err(StackError::Closed);
    }

    let len = self.storage.len();
    if len == self.storage.capacity() {
      let grown_to = self.handle_grow_policy(len + 1)?;
      if let Some(capacity) = grown_to {
        self.storage.push(item);
        return Ok(PushOutcome::GrewTo { capacity });
      }
    }

    self.storage.push(item);
    Ok(PushOutcome::Pushed)
  }

  fn pop(&mut self) -> Result<T, StackError> {
    match self.storage.pop() {
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
    self.storage.peek()
  }

  fn len(&self) -> usize {
    self.storage.len()
  }

  fn capacity(&self) -> usize {
    self.storage.capacity()
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
