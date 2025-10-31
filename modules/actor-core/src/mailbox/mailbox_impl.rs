use core::num::NonZeroUsize;

use cellactor_utils_core_rs::collections::queue::{QueueError, backend::OfferOutcome};
use portable_atomic::{AtomicBool, Ordering};

use super::{
  enqueue_outcome::EnqueueOutcome, mailbox_message::MailboxMessage, mailbox_offer_future::MailboxOfferFuture,
  mailbox_poll_future::MailboxPollFuture, map_system_queue_error, map_user_queue_error, queue_handles::QueueHandles,
};
use crate::{
  ActorRuntimeMutex,
  any_message::AnyMessage,
  mailbox_instrumentation::MailboxInstrumentation,
  mailbox_policy::{MailboxCapacity, MailboxOverflowStrategy, MailboxPolicy},
  send_error::SendError,
  system_message::SystemMessage,
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct Mailbox {
  policy:          MailboxPolicy,
  system:          QueueHandles<SystemMessage>,
  user:            QueueHandles<AnyMessage>,
  suspended:       AtomicBool,
  instrumentation: ActorRuntimeMutex<Option<MailboxInstrumentation>>,
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
      instrumentation: ActorRuntimeMutex::new(None),
    }
  }

  /// Installs instrumentation hooks for metrics emission.
  pub fn set_instrumentation(&self, instrumentation: MailboxInstrumentation) {
    *self.instrumentation.lock() = Some(instrumentation);
  }

  /// Enqueues a system message, bypassing suspension.
  ///
  /// # Errors
  ///
  /// Returns an error if the system queue is full or closed.
  pub fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError> {
    self.offer_system(message)
  }

  /// Attempts to enqueue a user message; returns a future when blocking is needed.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, full, or closed.
  pub fn enqueue_user(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
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
  pub fn enqueue_user_future(&self, message: AnyMessage) -> MailboxOfferFuture {
    MailboxOfferFuture::new(self.user.offer_blocking(message))
  }

  /// Returns a future that resolves when the next user message becomes available.
  pub fn poll_user_future(&self) -> MailboxPollFuture {
    MailboxPollFuture::new(self.user.poll_blocking())
  }

  /// Dequeues the next available message, prioritising system queue.
  #[must_use]
  pub fn dequeue(&self) -> Option<MailboxMessage> {
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
    message: AnyMessage,
    overflow: MailboxOverflowStrategy,
  ) -> Result<EnqueueOutcome, SendError> {
    match overflow {
      | MailboxOverflowStrategy::DropNewest => {
        if self.user.consumer.len() >= capacity {
          return Err(SendError::full(message));
        }
        self.offer_user(message)
      },
      | MailboxOverflowStrategy::DropOldest => {
        if self.user.consumer.len() >= capacity
          && let Ok(dropped) = self.user.poll()
        {
          drop(dropped);
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

  fn offer_user(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    match self.user.offer(message) {
      | Ok(outcome) => {
        Self::handle_offer_outcome(outcome);
        self.publish_metrics();
        Ok(EnqueueOutcome::Enqueued)
      },
      | Err(error) => Err(map_user_queue_error(error)),
    }
  }

  fn offer_system(&self, message: SystemMessage) -> Result<(), SendError> {
    match self.system.offer(message) {
      | Ok(outcome) => {
        Self::handle_offer_outcome(outcome);
        self.publish_metrics();
        Ok(())
      },
      | Err(error) => Err(map_system_queue_error(error)),
    }
  }

  fn poll_queue<T>(handles: &QueueHandles<T>) -> Option<T> {
    match handles.poll() {
      | Ok(message) => Some(message),
      | Err(QueueError::Empty) | Err(QueueError::Disconnected) => None,
      | Err(QueueError::WouldBlock) => None,
      | Err(QueueError::Full(_))
      | Err(QueueError::OfferError(_))
      | Err(QueueError::Closed(_))
      | Err(QueueError::AllocError(_)) => None,
    }
  }

  const fn handle_offer_outcome(outcome: OfferOutcome) {
    let _ = outcome;
    // TODO: instrumentation hook for telemetry and EventStream integration.
  }

  fn publish_metrics(&self) {
    let guard = self.instrumentation.lock();
    if let Some(instrumentation) = guard.as_ref() {
      instrumentation.publish(self.user_len(), self.system_len());
    }
  }
}
