use alloc::boxed::Box;
use core::marker::PhantomData;

use async_trait::async_trait;

use super::{AsyncStackBackend, AsyncStackBackendInternal, PushOutcome, StackError, SyncStackBackend};
use crate::collections::wait::{WaitError, WaitQueue, WaitShared};

/// Adapter that exposes a synchronous stack backend through the async backend trait.
pub struct SyncStackAsyncAdapter<T, B>
where
  B: SyncStackBackend<T>, {
  backend:      B,
  _pd:          PhantomData<T>,
  push_waiters: WaitQueue<StackError>,
  pop_waiters:  WaitQueue<StackError>,
}

impl<T, B> SyncStackAsyncAdapter<T, B>
where
  B: SyncStackBackend<T>,
{
  /// Creates a new adapter wrapping the provided backend instance.
  #[must_use]
  pub fn new(backend: B) -> Self {
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

  pub(crate) fn register_push_waiter(&mut self) -> Result<WaitShared<StackError>, WaitError> {
    self.push_waiters.register()
  }

  pub(crate) fn register_pop_waiter(&mut self) -> Result<WaitShared<StackError>, WaitError> {
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

impl<T, B> AsyncStackBackend<T> for SyncStackAsyncAdapter<T, B> where B: SyncStackBackend<T> {}

#[async_trait(?Send)]
impl<T, B> AsyncStackBackendInternal<T> for SyncStackAsyncAdapter<T, B>
where
  B: SyncStackBackend<T>,
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

  fn prepare_push_wait(&mut self) -> Result<Option<WaitShared<StackError>>, WaitError> {
    if self.backend.overflow_policy() == super::StackOverflowPolicy::Block && !self.backend.is_closed() {
      Ok(Some(self.register_push_waiter()?))
    } else {
      Ok(None)
    }
  }

  fn prepare_pop_wait(&mut self) -> Result<Option<WaitShared<StackError>>, WaitError> {
    if self.backend.is_closed() {
      Ok(None)
    } else {
      Ok(Some(self.register_pop_waiter()?))
    }
  }

  fn is_closed(&self) -> bool {
    self.backend.is_closed()
  }
}
