use alloc::boxed::Box;
use core::marker::PhantomData;

use async_trait::async_trait;

use super::{AsyncStackBackend, PushOutcome, StackBackend, StackError};
use crate::v2::collections::wait::{WaitHandle, WaitQueue};

/// Adapter that exposes a synchronous stack backend through the async backend trait.
pub struct SyncAdapterStackBackend<T, B>
where
  B: StackBackend<T>, {
  backend:      B,
  _pd:          PhantomData<T>,
  push_waiters: WaitQueue<StackError>,
  pop_waiters:  WaitQueue<StackError>,
}

impl<T, B> SyncAdapterStackBackend<T, B>
where
  B: StackBackend<T>,
{
  /// Creates a new adapter wrapping the provided backend instance.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self {
      backend,
      _pd: PhantomData,
      push_waiters: WaitQueue::<StackError>::new(),
      pop_waiters: WaitQueue::<StackError>::new(),
    }
  }

  /// Consumes the adapter and returns the inner backend.
  #[must_use]
  pub fn into_inner(self) -> B {
    self.backend
  }

  /// Provides immutable access to the wrapped backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }

  /// Provides mutable access to the wrapped backend.
  #[must_use]
  pub const fn backend_mut(&mut self) -> &mut B {
    &mut self.backend
  }

  pub(crate) fn register_push_waiter(&mut self) -> WaitHandle<StackError> {
    self.push_waiters.register()
  }

  pub(crate) fn register_pop_waiter(&mut self) -> WaitHandle<StackError> {
    self.pop_waiters.register()
  }

  pub(crate) fn notify_push_waiter(&mut self) {
    let _ = self.push_waiters.notify_success();
  }

  pub(crate) fn notify_pop_waiter(&mut self) {
    let _ = self.pop_waiters.notify_success();
  }

  pub(crate) fn fail_all_waiters(&mut self, error: StackError) {
    self.push_waiters.notify_error_all(error);
    self.pop_waiters.notify_error_all(error);
  }
}

#[async_trait(?Send)]
impl<T, B> AsyncStackBackend<T> for SyncAdapterStackBackend<T, B>
where
  B: StackBackend<T>,
{
  async fn push(&mut self, item: T) -> Result<PushOutcome, StackError> {
    let result = self.backend.push(item);
    if result.is_ok() {
      self.notify_pop_waiter();
    }
    result
  }

  async fn pop(&mut self) -> Result<T, StackError> {
    match self.backend.pop() {
      | Ok(item) => {
        self.notify_push_waiter();
        Ok(item)
      },
      | Err(err) => Err(err),
    }
  }

  fn peek(&self) -> Option<&T> {
    self.backend.peek()
  }

  async fn close(&mut self) -> Result<(), StackError> {
    self.backend.close();
    self.fail_all_waiters(StackError::Closed);
    Ok(())
  }

  fn len(&self) -> usize {
    self.backend.len()
  }

  fn capacity(&self) -> usize {
    self.backend.capacity()
  }

  fn prepare_push_wait(&mut self) -> Option<WaitHandle<StackError>> {
    if self.backend.overflow_policy() == super::StackOverflowPolicy::Block && !self.backend.is_closed() {
      Some(self.register_push_waiter())
    } else {
      None
    }
  }

  fn prepare_pop_wait(&mut self) -> Option<WaitHandle<StackError>> {
    if self.backend.is_closed() { None } else { Some(self.register_pop_waiter()) }
  }

  fn is_closed(&self) -> bool {
    self.backend.is_closed()
  }
}
