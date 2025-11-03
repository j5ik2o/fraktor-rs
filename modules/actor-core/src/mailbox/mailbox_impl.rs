//! Priority mailbox maintaining separate queues for system and user messages.

use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicBool, Ordering},
};

use cellactor_utils_core_rs::{
  collections::queue::{QueueError, backend::OfferOutcome},
  sync::{SyncMutexFamily, sync_mutex_like::SyncMutexLike},
};

use super::{
  mailbox_enqueue_outcome::EnqueueOutcome, mailbox_instrumentation::MailboxInstrumentation,
  mailbox_message::MailboxMessage, mailbox_offer_future::MailboxOfferFuture, mailbox_poll_future::MailboxPollFuture,
  mailbox_queue_handles::QueueHandles, map_system_queue_error, map_user_queue_error,
};
use crate::{
  MailboxCapacity, MailboxOverflowStrategy, MailboxPolicy, RuntimeToolbox, SendError, SystemMessage,
  any_message::AnyMessage,
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct Mailbox<TB: RuntimeToolbox + 'static> {
  policy:          MailboxPolicy,
  system:          QueueHandles<SystemMessage, TB>,
  user:            QueueHandles<AnyMessage<TB>, TB>,
  suspended:       AtomicBool,
  instrumentation: crate::ToolboxMutex<Option<MailboxInstrumentation<TB>>, TB>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for Mailbox<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for Mailbox<TB> {}

impl<TB> Mailbox<TB>
where
  TB: RuntimeToolbox + 'static,
{
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
      instrumentation: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
  }

  /// Installs instrumentation hooks for metrics emission.
  pub fn set_instrumentation(&self, instrumentation: MailboxInstrumentation<TB>) {
    *self.instrumentation.lock() = Some(instrumentation);
  }

  /// Enqueues a system message, bypassing suspension.
  ///
  /// # Errors
  ///
  /// Returns an error if the system message queue is full or closed.
  pub fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError<TB>> {
    self.offer_system(message)
  }

  /// Attempts to enqueue a user message; returns a future when blocking is needed.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, full, or closed.
  pub fn enqueue_user(&self, message: AnyMessage<TB>) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
    if self.is_suspended() {
      return Err(SendError::suspended(message));
    }

    match self.policy.capacity() {
      | MailboxCapacity::Bounded { capacity } => {
        self.enqueue_bounded_user(capacity.get(), message, self.policy.overflow())
      },
      | MailboxCapacity::Unbounded => self.offer_user(message),
    }
  }

  /// Returns a future that resolves when the provided user message is enqueued.
  pub fn enqueue_user_future(&self, message: AnyMessage<TB>) -> MailboxOfferFuture<TB> {
    MailboxOfferFuture::new(self.user.offer_blocking(message))
  }

  /// Returns a future that resolves when the next user message becomes available.
  pub fn poll_user_future(&self) -> MailboxPollFuture<TB> {
    MailboxPollFuture::new(self.user.poll_blocking())
  }

  /// Dequeues the next available message, prioritising system queue.
  #[must_use]
  pub fn dequeue(&self) -> Option<MailboxMessage<TB>> {
    if let Some(system) = Self::poll_queue(&self.system) {
      self.publish_metrics();
      return Some(MailboxMessage::System(system));
    }

    if self.is_suspended() {
      return None;
    }

    let result = Self::poll_queue(&self.user).map(MailboxMessage::User);
    if result.is_some() {
      self.publish_metrics();
    }
    result
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
    message: AnyMessage<TB>,
    overflow: MailboxOverflowStrategy,
  ) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
    match overflow {
      | MailboxOverflowStrategy::DropNewest => {
        if self.user.consumer.len() >= capacity {
          return Err(SendError::full(message));
        }
        self.offer_user(message)
      },
      | MailboxOverflowStrategy::DropOldest => {
        if self.user.consumer.len() >= capacity && self.user.poll().is_ok() {
          // drop oldest message
        }
        self.offer_user(message)
      },
      | MailboxOverflowStrategy::Grow => self.offer_user(message),
      | MailboxOverflowStrategy::Block => {
        if self.user.consumer.len() >= capacity {
          let future = MailboxOfferFuture::new(self.user.offer_blocking(message));
          return Ok(EnqueueOutcome::Pending(future));
        }
        self.offer_user(message)
      },
    }
  }

  fn offer_user(&self, message: AnyMessage<TB>) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
    match self.user.offer(message) {
      | Ok(outcome) => {
        Self::handle_offer_outcome(outcome);
        self.publish_metrics();
        Ok(EnqueueOutcome::Enqueued)
      },
      | Err(error) => Err(map_user_queue_error(error)),
    }
  }

  fn offer_system(&self, message: SystemMessage) -> Result<(), SendError<TB>> {
    match self.system.offer(message) {
      | Ok(outcome) => {
        Self::handle_offer_outcome(outcome);
        self.publish_metrics();
        Ok(())
      },
      | Err(error) => Err(map_system_queue_error(error)),
    }
  }

  fn poll_queue<T: Send + 'static>(handles: &QueueHandles<T, TB>) -> Option<T> {
    match handles.poll() {
      | Ok(message) => Some(message),
      | Err(QueueError::Empty) => None,
      | Err(QueueError::Disconnected) => None,
      | Err(QueueError::WouldBlock) => None,
      | Err(QueueError::Full(_))
      | Err(QueueError::OfferError(_))
      | Err(QueueError::Closed(_))
      | Err(QueueError::AllocError(_)) => None,
    }
  }

  const fn handle_offer_outcome(outcome: OfferOutcome) {
    let _ = outcome;
  }

  fn publish_metrics(&self) {
    let guard = self.instrumentation.lock();
    if let Some(instrumentation) = guard.as_ref() {
      instrumentation.publish(self.user_len(), self.system_len());
    }
  }
}
