//! Priority mailbox maintaining separate queues for system and user messages.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::num::NonZeroUsize;

use cellactor_utils_core_rs::{
  collections::queue::{QueueError, backend::OfferOutcome},
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{
  BackpressurePublisherGeneric, MailboxOfferFutureGeneric, MailboxPollFutureGeneric, MailboxStateEngine, QueueHandles,
  ScheduleHints, SystemQueue, mailbox_enqueue_outcome::EnqueueOutcome,
  mailbox_instrumentation::MailboxInstrumentationGeneric, mailbox_message::MailboxMessage, map_user_queue_error,
};
use crate::{
  NoStdToolbox, RuntimeToolbox,
  error::SendError,
  logging::LogLevel,
  mailbox::{capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy},
  messaging::{AnyMessageGeneric, SystemMessage},
  system::SystemStateGeneric,
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct MailboxGeneric<TB: RuntimeToolbox + 'static> {
  policy:          MailboxPolicy,
  system:          SystemQueue,
  user:            QueueHandles<AnyMessageGeneric<TB>, TB>,
  state:           MailboxStateEngine,
  instrumentation: crate::ToolboxMutex<Option<MailboxInstrumentationGeneric<TB>>, TB>,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for MailboxGeneric<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for MailboxGeneric<TB> {}

impl<TB> MailboxGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new mailbox using the provided policy.
  #[must_use]
  pub fn new(policy: MailboxPolicy) -> Self {
    let user_handles = QueueHandles::new_user(&policy);
    Self {
      policy,
      system: SystemQueue::new(),
      user: user_handles,
      state: MailboxStateEngine::new(),
      instrumentation: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
  }

  /// Installs instrumentation hooks for metrics emission.
  pub(crate) fn set_instrumentation(&self, instrumentation: MailboxInstrumentationGeneric<TB>) {
    *self.instrumentation.lock() = Some(instrumentation);
  }

  /// Returns the system state handle if instrumentation has been installed.
  pub(crate) fn system_state(&self) -> Option<ArcShared<SystemStateGeneric<TB>>> {
    self.instrumentation.lock().as_ref().map(|inst| inst.system_state())
  }

  /// Emits a log message tagged with this mailbox pid.
  pub(crate) fn emit_log(&self, level: LogLevel, message: impl Into<String>) {
    if let Some(instrumentation) = self.instrumentation.lock().as_ref() {
      instrumentation.emit_log(level, message);
    }
  }

  /// Installs a backpressure publisher used for dispatcher coordination.
  pub(crate) fn attach_backpressure_publisher(&self, publisher: BackpressurePublisherGeneric<TB>) {
    let mut guard = self.instrumentation.lock();
    if let Some(instrumentation) = guard.as_mut() {
      instrumentation.attach_backpressure_publisher(publisher);
    }
  }

  /// Enqueues a system message, bypassing suspension.
  ///
  /// # Errors
  ///
  /// Returns an error if the system message queue is full or closed.
  pub(crate) fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError<TB>> {
    self.system.push(message);
    self.publish_metrics();
    Ok(())
  }

  /// Attempts to enqueue a user message; returns a future when blocking is needed.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, full, or closed.
  #[cfg_attr(not(test), doc(hidden))]
  #[cfg_attr(test, allow(dead_code))]
  pub fn enqueue_user(&self, message: AnyMessageGeneric<TB>) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
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
  #[allow(dead_code)]
  pub(crate) fn enqueue_user_future(&self, message: AnyMessageGeneric<TB>) -> MailboxOfferFutureGeneric<TB> {
    MailboxOfferFutureGeneric::new(self.user.offer_blocking(message))
  }

  /// Returns a future that resolves when the next user message becomes available.
  #[allow(dead_code)]
  pub(crate) fn poll_user_future(&self) -> MailboxPollFutureGeneric<TB> {
    MailboxPollFutureGeneric::new(self.user.poll_blocking())
  }

  /// Dequeues the next available message, prioritising system queue.
  #[must_use]
  pub(crate) fn dequeue(&self) -> Option<MailboxMessage<TB>> {
    if let Some(system) = self.system.pop() {
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
  pub(crate) fn suspend(&self) {
    self.state.suspend();
  }

  /// Resumes user message consumption.
  pub(crate) fn resume(&self) {
    self.state.resume();
  }

  /// Requests scheduling based on provided hints; returns `true` when dispatcher execution should
  /// start.
  #[must_use]
  pub(crate) fn request_schedule(&self, hints: ScheduleHints) -> bool {
    self.state.request_schedule(hints)
  }

  /// Marks the mailbox as running so the next dequeue cycle can begin.
  pub(crate) fn set_running(&self) {
    self.state.set_running();
  }

  /// Clears the running flag and returns whether a pending reschedule must occur immediately.
  #[must_use]
  pub(crate) fn set_idle(&self) -> bool {
    self.state.set_idle()
  }

  /// Computes schedule hints from the current queue lengths and suspension state.
  #[must_use]
  pub(crate) fn current_schedule_hints(&self) -> ScheduleHints {
    ScheduleHints {
      has_system_messages: self.system_len() > 0,
      has_user_messages:   !self.is_suspended() && self.user_len() > 0,
      backpressure_active: false,
    }
  }

  /// Indicates whether the mailbox is currently suspended.
  #[must_use]
  pub(crate) fn is_suspended(&self) -> bool {
    self.state.is_suspended()
  }

  /// Returns the number of user messages awaiting processing.
  #[must_use]
  pub(crate) fn user_len(&self) -> usize {
    self.user.consumer.len()
  }

  /// Returns the number of system messages awaiting processing.
  #[must_use]
  pub(crate) fn system_len(&self) -> usize {
    self.system.len()
  }

  /// Returns the configured throughput limit.
  #[must_use]
  pub const fn throughput_limit(&self) -> Option<NonZeroUsize> {
    self.policy.throughput_limit()
  }

  fn enqueue_bounded_user(
    &self,
    capacity: usize,
    message: AnyMessageGeneric<TB>,
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
          let future = MailboxOfferFutureGeneric::new(self.user.offer_blocking(message));
          return Ok(EnqueueOutcome::Pending(future));
        }
        self.offer_user(message)
      },
    }
  }

  fn offer_user(&self, message: AnyMessageGeneric<TB>) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
    match self.user.offer(message) {
      | Ok(outcome) => {
        Self::handle_offer_outcome(outcome);
        self.publish_metrics();
        Ok(EnqueueOutcome::Enqueued)
      },
      | Err(error) => Err(map_user_queue_error(error)),
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
      | Err(QueueError::AllocError(_))
      | Err(QueueError::TimedOut(_)) => None,
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

/// Type alias for `MailboxGeneric` with the default `NoStdToolbox`.
pub type Mailbox = MailboxGeneric<NoStdToolbox>;
