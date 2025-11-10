#![allow(clippy::module_name_repetitions)]

extern crate alloc;

use alloc::collections::VecDeque;
use core::{cmp, future::Future, pin::Pin, task::{Context, Poll}};

use spin::Mutex;

use crate::{
  collections::{
    queue::{OfferOutcome, OverflowPolicy, QueueError},
    wait::{WaitQueue, WaitShared},
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

/// Double-ended queue backend supporting both FIFO and LIFO style operations.
pub struct DequeBackend<T> {
  state: ArcShared<DequeState<T>>,
}

impl<T> Clone for DequeBackend<T> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<T> DequeBackend<T> {
  /// Creates a new backend with the specified capacity and overflow policy.
  #[must_use]
  pub fn with_capacity(capacity: usize, policy: OverflowPolicy) -> Self {
    let state = DequeState::new(capacity, policy);
    Self { state: ArcShared::new(state) }
  }

  /// Adds an element to the back of the deque respecting the configured overflow policy.
  pub fn offer_back(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(item, DequeEdge::Back)
  }

  /// Adds an element to the front of the deque respecting the configured overflow policy.
  pub fn offer_front(&self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(item, DequeEdge::Front)
  }

  /// Removes an element from the front of the deque.
  pub fn poll_front(&self) -> Result<T, QueueError<T>> {
    self.state.poll(DequeEdge::Front)
  }

  /// Removes an element from the back of the deque.
  pub fn poll_back(&self) -> Result<T, QueueError<T>> {
    self.state.poll(DequeEdge::Back)
  }

  /// Returns the current length of the deque.
  #[must_use]
  pub fn len(&self) -> usize {
    self.state.len()
  }

  /// Returns the configured capacity of the deque.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.state.capacity()
  }

  /// Returns a future that waits until the value can be appended to the back of the deque.
  pub fn offer_back_blocking(&self, item: T) -> DequeOfferFuture<T> {
    DequeOfferFuture::new(self.state.clone(), item, DequeEdge::Back)
  }

  /// Returns a future that waits until the value can be prepended to the front of the deque.
  pub fn offer_front_blocking(&self, item: T) -> DequeOfferFuture<T> {
    DequeOfferFuture::new(self.state.clone(), item, DequeEdge::Front)
  }
}

#[derive(Clone, Copy)]
enum DequeEdge {
  Front,
  Back,
}

struct DequeState<T> {
  inner:             SpinSyncMutex<DequeInner<T>>,
  producer_waiters:  Mutex<WaitQueue<QueueError<T>>>,
  consumer_waiters:  Mutex<WaitQueue<QueueError<T>>>,
}

impl<T> DequeState<T> {
  fn new(capacity: usize, policy: OverflowPolicy) -> Self {
    Self {
      inner: SpinSyncMutex::new(DequeInner::new(capacity, policy)),
      producer_waiters: Mutex::new(WaitQueue::new()),
      consumer_waiters: Mutex::new(WaitQueue::new()),
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

  fn capacity(&self) -> usize {
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

/// Future returned when a deque needs to wait for capacity.
pub struct DequeOfferFuture<T> {
  state:  ArcShared<DequeState<T>>,
  item:   Option<T>,
  waiter: Option<WaitShared<QueueError<T>>>,
  edge:   DequeEdge,
}

impl<T> DequeOfferFuture<T> {
  const fn new(state: ArcShared<DequeState<T>>, item: T, edge: DequeEdge) -> Self {
    Self { state, item: Some(item), waiter: None, edge }
  }

  fn ensure_waiter(&mut self) -> &mut WaitShared<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is always Some after the branch above.
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
  }
}

impl<T> Future for DequeOfferFuture<T> {
  type Output = Result<OfferOutcome, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      if let Some(item) = this.item.take() {
        match this.state.offer(item, this.edge) {
          | Ok(outcome) => {
            this.waiter.take();
            return Poll::Ready(Ok(outcome));
          },
          | Err(QueueError::Full(returned)) => {
            this.item = Some(returned);
          },
          | Err(error) => {
            this.waiter.take();
            return Poll::Ready(Err(error));
          },
        }
      }

      let waiter = this.ensure_waiter();
      match Pin::new(waiter).poll(cx) {
        | Poll::Pending => return Poll::Pending,
        | Poll::Ready(Ok(())) => continue,
        | Poll::Ready(Err(error)) => {
          this.waiter.take();
          return Poll::Ready(Err(error));
        },
      }
    }
  }
}

impl<T> Unpin for DequeOfferFuture<T> {}
