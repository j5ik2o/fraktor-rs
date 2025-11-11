#![allow(clippy::module_name_repetitions)]

extern crate alloc;

use alloc::collections::VecDeque;
use core::cmp;

use crate::{
  collections::{
    queue::{OfferOutcome, OverflowPolicy, QueueError},
    wait::{WaitQueue, WaitShared},
  },
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

mod deque_offer_future;
#[cfg(test)]
mod tests;

pub use deque_offer_future::DequeOfferFuture;

/// Double-ended queue backend supporting both FIFO and LIFO style operations.
pub struct DequeBackendGeneric<T, TB: RuntimeToolbox + 'static = NoStdToolbox>
where
  T: Send + 'static, {
  state: ArcShared<DequeState<T, TB>>,
}

impl<T, TB> Clone for DequeBackendGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<T, TB> DequeBackendGeneric<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new backend with the specified capacity and overflow policy.
  #[must_use]
  pub fn with_capacity(capacity: usize, policy: OverflowPolicy) -> Self {
    let state = DequeState::new(capacity, policy);
    Self { state: ArcShared::new(state) }
  }

  /// Adds an element to the back of the deque respecting the configured overflow policy.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError::Full`] when the deque is at capacity and the policy forbids growth.
  pub fn offer_back(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(item, DequeEdge::Back)
  }

  /// Adds an element to the front of the deque respecting the configured overflow policy.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError::Full`] when the deque is at capacity and the policy forbids growth.
  pub fn offer_front(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(item, DequeEdge::Front)
  }

  /// Removes an element from the front of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError::Empty`] when the deque has no entries or [`QueueError::Closed`] if the
  /// deque was closed.
  pub fn poll_front(&self) -> Result<T, QueueError<T>> {
    self.state.poll(DequeEdge::Front)
  }

  /// Removes an element from the back of the deque.
  ///
  /// # Errors
  ///
  /// Returns [`QueueError::Empty`] when the deque has no entries or [`QueueError::Closed`] if the
  /// deque was closed.
  pub fn poll_back(&self) -> Result<T, QueueError<T>> {
    self.state.poll(DequeEdge::Back)
  }

  /// Returns the current length of the deque.
  #[must_use]
  pub fn len(&self) -> usize {
    self.state.len()
  }

  /// Indicates whether the deque currently stores any elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Returns the configured capacity of the deque.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.state.capacity()
  }

  /// Returns a future that waits until the value can be appended to the back of the deque.
  pub fn offer_back_blocking(&self, item: T) -> DequeOfferFuture<T, TB> {
    DequeOfferFuture::new(self.state.clone(), item, DequeEdge::Back)
  }

  /// Returns a future that waits until the value can be prepended to the front of the deque.
  pub fn offer_front_blocking(&self, item: T) -> DequeOfferFuture<T, TB> {
    DequeOfferFuture::new(self.state.clone(), item, DequeEdge::Front)
  }
}

/// Default Deque backend using the no_std toolbox.
pub type DequeBackend<T> = DequeBackendGeneric<T, NoStdToolbox>;

#[derive(Clone, Copy)]
enum DequeEdge {
  Front,
  Back,
}

struct DequeState<T, TB: RuntimeToolbox + 'static>
where
  T: Send + 'static, {
  inner:            ToolboxMutex<DequeInner<T>, TB>,
  producer_waiters: ToolboxMutex<WaitQueue<QueueError<T>>, TB>,
  consumer_waiters: ToolboxMutex<WaitQueue<QueueError<T>>, TB>,
}

impl<T, TB> DequeState<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn new(capacity: usize, policy: OverflowPolicy) -> Self {
    Self {
      inner:            <TB::MutexFamily as SyncMutexFamily>::create(DequeInner::new(capacity, policy)),
      producer_waiters: <TB::MutexFamily as SyncMutexFamily>::create(WaitQueue::new()),
      consumer_waiters: <TB::MutexFamily as SyncMutexFamily>::create(WaitQueue::new()),
    }
  }

  fn offer(&self, item: T, edge: DequeEdge) -> Result<OfferOutcome, QueueError<T>> {
    let result = {
      let mut guard = self.inner.lock();
      guard.offer(item, edge)
    };

    if result.is_ok() {
      self.notify_consumer_waiter();
    }

    result
  }

  fn poll(&self, edge: DequeEdge) -> Result<T, QueueError<T>> {
    let result = {
      let mut guard = self.inner.lock();
      guard.poll(edge)
    };

    if result.is_ok() {
      self.notify_producer_waiter();
    }

    result
  }

  fn len(&self) -> usize {
    self.inner.lock().len()
  }

  fn capacity(&self) -> usize {
    self.inner.lock().capacity()
  }

  fn register_producer_waiter(&self) -> WaitShared<QueueError<T>> {
    self.producer_waiters.lock().register()
  }

  fn notify_producer_waiter(&self) {
    let _ = self.producer_waiters.lock().notify_success();
  }

  fn notify_consumer_waiter(&self) {
    let _ = self.consumer_waiters.lock().notify_success();
  }
}

struct DequeInner<T> {
  buffer:   VecDeque<T>,
  capacity: usize,
  policy:   OverflowPolicy,
  closed:   bool,
}

impl<T> DequeInner<T> {
  const fn new(capacity: usize, policy: OverflowPolicy) -> Self {
    Self { buffer: VecDeque::new(), capacity, policy, closed: false }
  }

  fn len(&self) -> usize {
    self.buffer.len()
  }

  const fn capacity(&self) -> usize {
    self.capacity
  }

  fn offer(&mut self, item: T, edge: DequeEdge) -> Result<OfferOutcome, QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(item));
    }

    if self.len() >= self.capacity {
      return self.handle_full(item, edge);
    }

    match edge {
      | DequeEdge::Front => self.buffer.push_front(item),
      | DequeEdge::Back => self.buffer.push_back(item),
    }

    Ok(OfferOutcome::Enqueued)
  }

  fn poll(&mut self, edge: DequeEdge) -> Result<T, QueueError<T>> {
    let value = match edge {
      | DequeEdge::Front => self.buffer.pop_front(),
      | DequeEdge::Back => self.buffer.pop_back(),
    };

    match value {
      | Some(item) => Ok(item),
      | None => {
        if self.closed {
          Err(QueueError::Disconnected)
        } else {
          Err(QueueError::Empty)
        }
      },
    }
  }

  fn handle_full(&mut self, item: T, edge: DequeEdge) -> Result<OfferOutcome, QueueError<T>> {
    match self.policy {
      | OverflowPolicy::DropNewest => {
        drop(item);
        Ok(OfferOutcome::DroppedNewest { count: 1 })
      },
      | OverflowPolicy::DropOldest => {
        let _ = self.buffer.pop_front();
        match edge {
          | DequeEdge::Front => self.buffer.push_front(item),
          | DequeEdge::Back => self.buffer.push_back(item),
        }
        Ok(OfferOutcome::DroppedOldest { count: 1 })
      },
      | OverflowPolicy::Block => Err(QueueError::Full(item)),
      | OverflowPolicy::Grow => {
        self.grow();
        match edge {
          | DequeEdge::Front => self.buffer.push_front(item),
          | DequeEdge::Back => self.buffer.push_back(item),
        }
        Ok(OfferOutcome::GrewTo { capacity: self.capacity })
      },
    }
  }

  fn grow(&mut self) {
    let next = cmp::max(1, self.capacity.saturating_mul(2));
    self.capacity = next;
  }
}

impl<T> Drop for DequeInner<T> {
  fn drop(&mut self) {
    self.buffer.clear();
  }
}
