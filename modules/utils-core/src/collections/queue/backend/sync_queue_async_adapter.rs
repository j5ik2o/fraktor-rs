use alloc::boxed::Box;
use core::marker::PhantomData;

use async_trait::async_trait;

use super::{
  AsyncPriorityBackend, AsyncQueueBackend, OfferOutcome, SyncQueueBackend, sync_priority_backend::SyncPriorityBackend,
};
use crate::collections::{
  PriorityMessage,
  queue::QueueError,
  wait::{WaitQueue, WaitShared},
};

/// Adapter that exposes a synchronous queue backend through the async backend trait.
///
/// # Warning
///
/// This adapter is meant to be constructed and driven by `AsyncQueue`/`SyncQueue`
/// helpers. Prefer those high-level APIs and implement custom backends instead of
/// invoking this adapter directly from application logic.
pub struct SyncQueueAsyncAdapter<T, B>
where
  B: SyncQueueBackend<T>, {
  backend:          B,
  _pd:              PhantomData<T>,
  producer_waiters: WaitQueue<QueueError<T>>,
  consumer_waiters: WaitQueue<QueueError<T>>,
}

impl<T, B> SyncQueueAsyncAdapter<T, B>
where
  B: SyncQueueBackend<T>,
{
  /// Creates a new adapter wrapping the provided backend instance.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self {
      backend,
      _pd: PhantomData,
      producer_waiters: WaitQueue::<QueueError<T>>::new(),
      consumer_waiters: WaitQueue::<QueueError<T>>::new(),
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

  pub(crate) fn register_producer_waiter(&mut self) -> WaitShared<QueueError<T>> {
    self.producer_waiters.register()
  }

  pub(crate) fn register_consumer_waiter(&mut self) -> WaitShared<QueueError<T>> {
    self.consumer_waiters.register()
  }

  pub(crate) fn notify_producer_waiter(&mut self) {
    let _ = self.producer_waiters.notify_success();
  }

  pub(crate) fn notify_consumer_waiter(&mut self) {
    let _ = self.consumer_waiters.notify_success();
  }

  pub(crate) fn fail_all_waiters<F>(&mut self, mut make_error: F)
  where
    F: FnMut() -> QueueError<T>, {
    self.producer_waiters.notify_error_all_with(&mut make_error);
    self.consumer_waiters.notify_error_all_with(&mut make_error);
  }
}

#[async_trait(?Send)]
impl<T, B> AsyncQueueBackend<T> for SyncQueueAsyncAdapter<T, B>
where
  B: SyncQueueBackend<T>,
{
  async fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = self.backend.offer(item);
    if result.is_ok() {
      self.notify_consumer_waiter();
    }
    result
  }

  async fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.backend.poll() {
      | Ok(item) => {
        self.notify_producer_waiter();
        Ok(item)
      },
      | Err(err) => Err(err),
    }
  }

  async fn close(&mut self) -> Result<(), QueueError<T>> {
    self.backend.close();
    self.fail_all_waiters(|| QueueError::Disconnected);
    Ok(())
  }

  fn len(&self) -> usize {
    self.backend.len()
  }

  fn capacity(&self) -> usize {
    self.backend.capacity()
  }

  fn prepare_producer_wait(&mut self) -> Option<WaitShared<QueueError<T>>> {
    if self.backend.overflow_policy() == super::OverflowPolicy::Block && !self.backend.is_closed() {
      Some(self.register_producer_waiter())
    } else {
      None
    }
  }

  fn prepare_consumer_wait(&mut self) -> Option<WaitShared<QueueError<T>>> {
    if self.backend.is_closed() { None } else { Some(self.register_consumer_waiter()) }
  }

  fn is_closed(&self) -> bool {
    self.backend.is_closed()
  }
}

impl<T: PriorityMessage, B> AsyncPriorityBackend<T> for SyncQueueAsyncAdapter<T, B>
where
  B: SyncPriorityBackend<T>,
{
  fn peek_min(&self) -> Option<&T> {
    self.backend.peek_min()
  }
}
