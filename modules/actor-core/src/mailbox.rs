//! Priority mailbox managing system and user message queues.

use core::{cmp, fmt, future::Future, num::NonZeroUsize, pin::Pin, task::{Context, Poll}};

use cellactor_utils_core_rs::{
  collections::{
    queue::{
      backend::{OfferOutcome, OverflowPolicy, VecRingBackend},
      MpscQueue, SyncMpscConsumer, SyncMpscProducer, VecRingStorage, SyncQueueBackend,
    },
    queue_old::QueueError,
    wait::{WaitHandle, WaitQueue},
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};
use portable_atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::{
  any_message::AnyOwnedMessage,
  mailbox_policy::{MailboxCapacity, MailboxOverflowStrategy, MailboxPolicy},
  send_error::SendError,
  system_message::SystemMessage,
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;
const SYSTEM_QUEUE_CAPACITY: usize = 8;

type QueueMutex<T> = SpinSyncMutex<VecRingBackend<T>>;
type QueueProducer<T> = SyncMpscProducer<T, VecRingBackend<T>, QueueMutex<T>>;
type QueueConsumer<T> = SyncMpscConsumer<T, VecRingBackend<T>, QueueMutex<T>>;

struct QueueState<T> {
  shared:             ArcShared<QueueMutex<T>>,
  producer_waiters:   Mutex<WaitQueue<QueueError<T>>>,
  consumer_waiters:   Mutex<WaitQueue<QueueError<T>>>,
}

impl<T> QueueState<T> {
  fn new(shared: ArcShared<QueueMutex<T>>) -> Self {
    Self { shared, producer_waiters: Mutex::new(WaitQueue::new()), consumer_waiters: Mutex::new(WaitQueue::new()) }
  }

  fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.offer(message)
    };

    if result.is_ok() {
      self.notify_consumer_waiter();
    }

    result
  }

  fn poll(&self) -> Result<T, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.poll()
    };

    if result.is_ok() {
      self.notify_producer_waiter();
    }

    result
  }

  fn register_producer_waiter(&self) -> WaitHandle<QueueError<T>> {
    self.producer_waiters.lock().register()
  }

  fn register_consumer_waiter(&self) -> WaitHandle<QueueError<T>> {
    self.consumer_waiters.lock().register()
  }

  fn notify_producer_waiter(&self) {
    let _ = self.producer_waiters.lock().notify_success();
  }

  fn notify_consumer_waiter(&self) {
    let _ = self.consumer_waiters.lock().notify_success();
  }
}

/// Future returned when a queue needs to wait for capacity.
pub struct QueueOfferFuture<T> {
  state:  ArcShared<QueueState<T>>,
  message: Option<T>,
  waiter: Option<WaitHandle<QueueError<T>>>,
}

impl<T> QueueOfferFuture<T> {
  fn new(state: ArcShared<QueueState<T>>, message: T) -> Self {
    Self { state, message: Some(message), waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitHandle<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter();
      self.waiter = Some(waiter);
    }
    self.waiter.as_mut().expect("waiter must be present")
  }
}

impl<T> Unpin for QueueOfferFuture<T> {}

impl<T> Future for QueueOfferFuture<T> {
  type Output = Result<OfferOutcome, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      if let Some(message) = this.message.take() {
        match this.state.offer(message) {
          Ok(outcome) => {
            this.waiter.take();
            return Poll::Ready(Ok(outcome));
          },
          Err(QueueError::Full(item)) => {
            this.message = Some(item);
          },
          Err(error) => {
            this.waiter.take();
            return Poll::Ready(Err(error));
          },
        }
      }

      let waiter = this.ensure_waiter();
      match Pin::new(waiter).poll(cx) {
        Poll::Pending => return Poll::Pending,
        Poll::Ready(Ok(())) => continue,
        Poll::Ready(Err(error)) => {
          this.waiter.take();
          return Poll::Ready(Err(error));
        },
      }
    }
  }
}

/// Future returned when a queue needs to wait for incoming messages.
pub struct QueuePollFuture<T> {
  state:  ArcShared<QueueState<T>>,
  waiter: Option<WaitHandle<QueueError<T>>>,
}

impl<T> QueuePollFuture<T> {
  fn new(state: ArcShared<QueueState<T>>) -> Self {
    Self { state, waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitHandle<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_consumer_waiter();
      self.waiter = Some(waiter);
    }
    self.waiter.as_mut().expect("waiter must be present")
  }
}

impl<T> Unpin for QueuePollFuture<T> {}

impl<T> Future for QueuePollFuture<T> {
  type Output = Result<T, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      match this.state.poll() {
        Ok(item) => {
          this.waiter.take();
          return Poll::Ready(Ok(item));
        },
        Err(QueueError::Empty) => {
          let waiter = this.ensure_waiter();
          match Pin::new(waiter).poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Ok(())) => continue,
            Poll::Ready(Err(error)) => {
              this.waiter.take();
              return Poll::Ready(Err(error));
            },
          }
        },
        Err(error) => {
          this.waiter.take();
          return Poll::Ready(Err(error));
        },
      }
    }
  }
}

/// Future specialized for mailbox user queue offers.
pub struct MailboxOfferFuture {
  inner: QueueOfferFuture<AnyOwnedMessage>,
}

impl MailboxOfferFuture {
  fn new(inner: QueueOfferFuture<AnyOwnedMessage>) -> Self {
    Self { inner }
  }
}

impl Unpin for MailboxOfferFuture {}

impl Future for MailboxOfferFuture {
  type Output = Result<(), SendError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl fmt::Debug for MailboxOfferFuture {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxOfferFuture").finish()
  }
}

/// Future specialized for mailbox user queue polling.
pub struct MailboxPollFuture {
  inner: QueuePollFuture<AnyOwnedMessage>,
}

impl MailboxPollFuture {
  fn new(inner: QueuePollFuture<AnyOwnedMessage>) -> Self {
    Self { inner }
  }
}

impl Unpin for MailboxPollFuture {}

impl Future for MailboxPollFuture {
  type Output = Result<AnyOwnedMessage, SendError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      Poll::Ready(Ok(message)) => Poll::Ready(Ok(message)),
      Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl fmt::Debug for MailboxPollFuture {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxPollFuture").finish()
  }
}

/// Outcome returned by immediate enqueue attempts.
#[derive(Debug)]
pub enum EnqueueOutcome {
  /// The message was enqueued immediately.
  Enqueued,
  /// The mailbox is full and a future must be awaited for completion.
  Pending(MailboxOfferFuture),
}

/// Represents messages dequeued from the mailbox.
#[derive(Debug)]
pub enum MailboxMessage {
  /// Internal system-level message.
  System(SystemMessage),
  /// Application user-level message.
  User(AnyOwnedMessage),
}

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct Mailbox {
  policy:       MailboxPolicy,
  system:       QueueHandles<SystemMessage>,
  user:         QueueHandles<AnyOwnedMessage>,
  suspended:    AtomicBool,
}

impl Mailbox {
  /// Creates a new mailbox using the provided policy.
  #[must_use]
  pub fn new(policy: MailboxPolicy) -> Self {
    let user_handles = QueueHandles::new_user(&policy);
    let system_handles = QueueHandles::new_system();
    Self {
      policy,
      system: system_handles,
      user: user_handles,
      suspended: AtomicBool::new(false),
    }
  }

  /// Enqueues a system message, bypassing suspension.
  pub fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError> {
    self.offer_system(message)
  }

  /// Attempts to enqueue a user message; returns a future when Block ポリシーで待機が必要。
  pub fn enqueue_user(&self, message: AnyOwnedMessage) -> Result<EnqueueOutcome, SendError> {
    if self.is_suspended() {
      return Err(SendError::suspended(message));
    }

    match self.policy.capacity() {
      MailboxCapacity::Bounded { capacity } => {
        self.enqueue_bounded_user(capacity.get(), message, self.policy.overflow())
      },
      MailboxCapacity::Unbounded => self.offer_user(message),
    }
  }

  /// Returns a future that resolves when the provided user message is enqueued.
  pub fn enqueue_user_future(&self, message: AnyOwnedMessage) -> MailboxOfferFuture {
    MailboxOfferFuture::new(self.user.offer_blocking(message))
  }

  /// Returns a future that resolves when the next user message becomes available.
  pub fn poll_user_future(&self) -> MailboxPollFuture {
    MailboxPollFuture::new(self.user.poll_blocking())
  }

  /// Dequeues the next available message, prioritising system queue.
  #[must_use]
  pub fn dequeue(&self) -> Option<MailboxMessage> {
    if let Some(system) = self.poll_queue(&self.system) {
      return Some(MailboxMessage::System(system));
    }

    if self.is_suspended() {
      return None;
    }

    self.poll_queue(&self.user).map(MailboxMessage::User)
  }

  /// Suspends user message consumption.
  pub fn suspend(&self) {
    self.suspended.store(true, Ordering::Release);
  }

  /// Resumes user message consumption.
  pub fn resume(&self) {
    self.suspended.store(false, Ordering::Release);
  }

  /// Indicates whether the mailbox is currently suspended.
  #[must_use]
  pub fn is_suspended(&self) -> bool {
    self.suspended.load(Ordering::Acquire)
  }

  /// Returns the number of user messages awaiting processing.
  #[must_use]
  pub fn user_len(&self) -> usize {
    self.user.consumer.len()
  }

  /// Returns the number of system messages awaiting processing.
  #[must_use]
  pub fn system_len(&self) -> usize {
    self.system.consumer.len()
  }

  /// Returns the configured throughput limit.
  #[must_use]
  pub const fn throughput_limit(&self) -> Option<NonZeroUsize> {
    self.policy.throughput_limit()
  }

  fn enqueue_bounded_user(
    &self,
    capacity: usize,
    message: AnyOwnedMessage,
    overflow: MailboxOverflowStrategy,
  ) -> Result<EnqueueOutcome, SendError> {
    match overflow {
      MailboxOverflowStrategy::DropNewest => {
        if self.user.consumer.len() >= capacity {
          return Err(SendError::full(message));
        }
        self.offer_user(message)
      },
      MailboxOverflowStrategy::DropOldest => {
        if self.user.consumer.len() >= capacity {
          if let Ok(dropped) = self.user.poll() {
            drop(dropped);
          }
        }
        self.offer_user(message)
      },
      MailboxOverflowStrategy::Grow => self.offer_user(message),
      MailboxOverflowStrategy::Block => {
        if self.user.consumer.len() >= capacity {
          let future = MailboxOfferFuture::new(self.user.offer_blocking(message));
          return Ok(EnqueueOutcome::Pending(future));
        }
        self.offer_user(message)
      },
    }
  }

  fn offer_user(&self, message: AnyOwnedMessage) -> Result<EnqueueOutcome, SendError> {
    match self.user.offer(message) {
      Ok(outcome) => {
        self.handle_offer_outcome(outcome);
        Ok(EnqueueOutcome::Enqueued)
      },
      Err(error) => Err(map_user_queue_error(error)),
    }
  }

  fn offer_system(&self, message: SystemMessage) -> Result<(), SendError> {
    match self.system.offer(message) {
      Ok(outcome) => {
        self.handle_offer_outcome(outcome);
        Ok(())
      },
      Err(error) => Err(map_system_queue_error(error)),
    }
  }

  fn poll_queue<T>(&self, handles: &QueueHandles<T>) -> Option<T> {
    match handles.poll() {
      Ok(message) => Some(message),
      Err(QueueError::Empty) | Err(QueueError::Disconnected) => None,
      Err(QueueError::WouldBlock) => None,
      Err(QueueError::Full(_))
      | Err(QueueError::OfferError(_))
      | Err(QueueError::Closed(_))
      | Err(QueueError::AllocError(_)) => None,
    }
  }

  fn handle_offer_outcome(&self, outcome: OfferOutcome) {
    let _ = outcome;
    // TODO: instrumentation hook for telemetry and EventStream integration.
  }
}

struct QueueHandles<T> {
  state:      ArcShared<QueueState<T>>,
  _producer:  QueueProducer<T>,
  consumer:   QueueConsumer<T>,
}

impl<T> QueueHandles<T> {
  fn new_user(policy: &MailboxPolicy) -> Self {
    let (capacity, overflow) = match policy.capacity() {
      MailboxCapacity::Bounded { capacity } => {
        (cmp::max(1, capacity.get()), map_overflow(policy.overflow()))
      },
      MailboxCapacity::Unbounded => (DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow),
    };
    QueueHandles::new_with(capacity, overflow)
  }

  fn new_system() -> Self {
    QueueHandles::new_with(SYSTEM_QUEUE_CAPACITY, OverflowPolicy::Grow)
  }

  fn new_with(capacity: usize, overflow: OverflowPolicy) -> Self {
    let storage = VecRingStorage::with_capacity(capacity);
    let backend = VecRingBackend::new_with_storage(storage, overflow);
    let mutex = SpinSyncMutex::new(backend);
    let shared = ArcShared::new(mutex);
    let state = ArcShared::new(QueueState::new(shared.clone()));
    let queue: MpscQueue<_, VecRingBackend<T>, _> = MpscQueue::new_mpsc(shared);
    let (producer, consumer) = queue.into_mpsc_pair();
    Self { state, _producer: producer, consumer }
  }

  fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(message)
  }

  fn poll(&self) -> Result<T, QueueError<T>> {
    self.state.poll()
  }

  fn offer_blocking(&self, message: T) -> QueueOfferFuture<T> {
    QueueOfferFuture::new(self.state.clone(), message)
  }

  fn poll_blocking(&self) -> QueuePollFuture<T> {
    QueuePollFuture::new(self.state.clone())
  }

}

fn map_user_queue_error(error: QueueError<AnyOwnedMessage>) -> SendError {
  match error {
    QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(item),
    QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(item),
    QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}

fn map_system_queue_error(error: QueueError<SystemMessage>) -> SendError {
  match error {
    QueueError::Full(item) | QueueError::OfferError(item) => SendError::full(AnyOwnedMessage::new(item)),
    QueueError::Closed(item) | QueueError::AllocError(item) => SendError::closed(AnyOwnedMessage::new(item)),
    QueueError::Disconnected | QueueError::Empty | QueueError::WouldBlock => {
      panic!("unexpected queue error variant during offer")
    },
  }
}

fn map_overflow(strategy: MailboxOverflowStrategy) -> OverflowPolicy {
  match strategy {
    MailboxOverflowStrategy::DropNewest => OverflowPolicy::DropNewest,
    MailboxOverflowStrategy::DropOldest => OverflowPolicy::DropOldest,
    MailboxOverflowStrategy::Grow => OverflowPolicy::Grow,
    MailboxOverflowStrategy::Block => OverflowPolicy::Block,
  }
}

#[cfg(test)]
mod tests;
