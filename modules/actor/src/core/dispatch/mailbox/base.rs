//! Priority mailbox maintaining separate queues for system and user messages.

#[cfg(test)]
mod tests;

use alloc::{collections::VecDeque, string::String};
use core::num::NonZeroUsize;

use fraktor_utils_rs::core::{
  collections::queue::{OfferOutcome, QueueError},
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::sync_mutex_like::SyncMutexLike,
};

use super::{
  BackpressurePublisherGeneric, MailboxOfferFutureGeneric, MailboxScheduleState, QueueStateHandle, ScheduleHints,
  SystemQueue, mailbox_enqueue_outcome::EnqueueOutcome, mailbox_instrumentation::MailboxInstrumentationGeneric,
  mailbox_message::MailboxMessage, map_user_queue_error,
};
use crate::core::{
  actor::Pid,
  dispatch::mailbox::{capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy},
  error::SendError,
  event::logging::LogLevel,
  messaging::{AnyMessageGeneric, system_message::SystemMessage},
  system::state::SystemStateSharedGeneric,
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct MailboxGeneric<TB: RuntimeToolbox + 'static> {
  policy:          MailboxPolicy,
  system:          SystemQueue,
  user:            QueueStateHandle<AnyMessageGeneric<TB>, TB>,
  state:           MailboxScheduleState,
  instrumentation: ToolboxMutex<Option<MailboxInstrumentationGeneric<TB>>, TB>,
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
    let user_handles = QueueStateHandle::new_user(&policy);
    Self {
      policy,
      system: SystemQueue::new(),
      user: user_handles,
      state: MailboxScheduleState::new(),
      instrumentation: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
  }

  /// Installs instrumentation hooks for metrics emission.
  pub(crate) fn set_instrumentation(&self, instrumentation: MailboxInstrumentationGeneric<TB>) {
    *self.instrumentation.lock() = Some(instrumentation);
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub(crate) const fn policy(&self) -> &MailboxPolicy {
    &self.policy
  }

  /// Returns the system state handle if instrumentation has been installed.
  pub(crate) fn system_state(&self) -> Option<SystemStateSharedGeneric<TB>> {
    self.instrumentation.lock().as_ref().and_then(|inst| inst.system_state())
  }

  /// Returns the actor pid associated with this mailbox when instrumentation is installed.
  #[must_use]
  pub(crate) fn pid(&self) -> Option<Pid> {
    self.instrumentation.lock().as_ref().map(|inst| inst.pid())
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
  #[allow(clippy::unnecessary_wraps)]
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

  /// Prepends user messages so they are processed before already queued user messages.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, capacity checks fail, or queue restoration
  /// fails.
  pub(crate) fn prepend_user_messages(&self, messages: &VecDeque<AnyMessageGeneric<TB>>) -> Result<(), SendError<TB>> {
    let Some(first_message) = messages.front().cloned() else {
      return Ok(());
    };

    if self.is_suspended() {
      return Err(SendError::suspended(first_message));
    }

    if self.prepend_would_overflow(messages.len()) {
      return Err(SendError::full(first_message));
    }

    let mut state = self.user.state.lock();
    let mut existing = VecDeque::new();
    loop {
      match state.poll() {
        | Ok(message) => existing.push_back(message),
        | Err(QueueError::Empty | QueueError::Disconnected | QueueError::WouldBlock) => break,
        | Err(QueueError::Full(_))
        | Err(QueueError::OfferError(_))
        | Err(QueueError::Closed(_))
        | Err(QueueError::AllocError(_))
        | Err(QueueError::TimedOut(_)) => {
          drop(state);
          self.publish_metrics();
          return Err(SendError::closed(first_message));
        },
      }
    }

    let mut inserted = 0_usize;
    let mut insertion_error = None;
    for message in messages.iter().cloned().chain(existing.iter().cloned()) {
      match state.offer(message) {
        | Ok(outcome) => {
          Self::handle_offer_outcome(outcome);
          inserted += 1;
        },
        | Err(error) => {
          insertion_error = Some(map_user_queue_error(error));
          break;
        },
      }
    }

    if let Some(error) = insertion_error {
      for _ in 0..inserted {
        let _ = state.poll();
      }
      for message in existing.iter().cloned() {
        if let Err(restore_error) = state.offer(message) {
          drop(state);
          self.publish_metrics();
          return Err(map_user_queue_error(restore_error));
        }
      }
      drop(state);
      self.publish_metrics();
      return Err(error);
    }

    drop(state);
    self.publish_metrics();
    Ok(())
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

  /// Indicates whether the mailbox is currently in a running state.
  #[must_use]
  pub(crate) fn is_running(&self) -> bool {
    self.state.is_running()
  }

  /// Computes schedule hints from the current queue lengths and suspension state.
  #[must_use]
  pub(crate) fn current_schedule_hints(&self) -> ScheduleHints {
    ScheduleHints {
      has_system_messages: !self.system.is_empty(),
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
    self.user.len()
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

  fn prepend_would_overflow(&self, prepended_count: usize) -> bool {
    let MailboxCapacity::Bounded { capacity } = self.policy.capacity() else {
      return false;
    };

    if matches!(self.policy.overflow(), MailboxOverflowStrategy::Grow) {
      return false;
    }

    self.user_len().saturating_add(prepended_count) > capacity.get()
  }

  fn enqueue_bounded_user(
    &self,
    capacity: usize,
    message: AnyMessageGeneric<TB>,
    overflow: MailboxOverflowStrategy,
  ) -> Result<EnqueueOutcome<TB>, SendError<TB>> {
    match overflow {
      | MailboxOverflowStrategy::DropNewest => {
        let len = self.user.len();
        if len >= capacity {
          return Err(SendError::full(message));
        }
        self.offer_user(message)
      },
      | MailboxOverflowStrategy::DropOldest => {
        if self.user.len() >= capacity && self.user.poll().is_ok() {
          // drop oldest message
        }
        self.offer_user(message)
      },
      | MailboxOverflowStrategy::Grow => self.offer_user(message),
      | MailboxOverflowStrategy::Block => {
        if self.user.len() >= capacity {
          let future = MailboxOfferFutureGeneric::new(self.user.state.clone(), message);
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

  fn poll_queue<T: Send + 'static>(handles: &QueueStateHandle<T, TB>) -> Option<T> {
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
