//! Priority mailbox maintaining separate queues for system and user messages.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, string::String};
use core::num::NonZeroUsize;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{
  BackpressurePublisher, MailboxScheduleState, ScheduleHints, SystemQueue, mailbox_enqueue_outcome::EnqueueOutcome,
  mailbox_instrumentation::MailboxInstrumentation, mailbox_message::MailboxMessage, message_queue::MessageQueue,
};
use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    props::MailboxConfig,
  },
  dispatch::mailbox::{capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy},
  event::logging::LogLevel,
  system::state::SystemStateShared,
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct Mailbox {
  policy:          MailboxPolicy,
  system:          SystemQueue,
  user:            Box<dyn MessageQueue>,
  user_queue_lock: ArcShared<RuntimeMutex<()>>,
  state:           MailboxScheduleState,
  instrumentation: ArcShared<RuntimeMutex<Option<MailboxInstrumentation>>>,
}

unsafe impl Send for Mailbox {}
unsafe impl Sync for Mailbox {}

impl Mailbox {
  /// Creates a new mailbox using the provided policy.
  #[must_use]
  pub fn new(policy: MailboxPolicy) -> Self {
    let queue = super::mailboxes::create_message_queue_from_policy(policy);
    Self::new_with_queue(policy, queue)
  }

  /// Creates a new mailbox from the provided configuration.
  ///
  /// When the config declares deque semantics and the policy is unbounded, this produces a
  /// deque-capable queue that supports O(1) front insertion in
  /// [`prepend_user_messages`](Self::prepend_user_messages).
  ///
  /// # Errors
  ///
  /// Returns [`MailboxConfigError`](crate::core::kernel::actor::props::MailboxConfigError) when the
  /// configuration contract is violated.
  pub fn new_from_config(
    config: &MailboxConfig,
  ) -> Result<Self, crate::core::kernel::actor::props::MailboxConfigError> {
    let policy = config.policy();
    let queue = super::mailboxes::create_message_queue_from_config(config)?;
    Ok(Self::new_with_queue(policy, queue))
  }

  /// Creates a new mailbox using the provided policy and pre-built queue.
  #[must_use]
  pub(crate) fn new_with_queue(policy: MailboxPolicy, queue: Box<dyn MessageQueue>) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      user_queue_lock: ArcShared::new(RuntimeMutex::new(())),
      state: MailboxScheduleState::new(),
      instrumentation: ArcShared::new(RuntimeMutex::new(None)),
    }
  }

  /// Installs instrumentation hooks for metrics emission.
  pub(crate) fn set_instrumentation(&self, instrumentation: MailboxInstrumentation) {
    *self.instrumentation.lock() = Some(instrumentation);
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub(crate) const fn policy(&self) -> &MailboxPolicy {
    &self.policy
  }

  /// Returns the system state handle if instrumentation has been installed.
  pub(crate) fn system_state(&self) -> Option<SystemStateShared> {
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
  pub(crate) fn attach_backpressure_publisher(&self, publisher: BackpressurePublisher) {
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
  pub(crate) fn enqueue_system(&self, message: SystemMessage) -> Result<(), SendError> {
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
  pub fn enqueue_user(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    if self.is_suspended() {
      return Err(SendError::suspended(message));
    }

    let enqueue_result = {
      let _guard = self.user_queue_lock.lock();
      self.user.enqueue(message)
    };

    match enqueue_result {
      | Ok(EnqueueOutcome::Enqueued) => {
        self.publish_metrics();
        Ok(EnqueueOutcome::Enqueued)
      },
      | Ok(EnqueueOutcome::Pending(future)) => {
        let future = future
          .with_user_queue_lock(self.user_queue_lock.clone())
          .with_metrics(self.instrumentation.clone(), self.system.len_handle());
        Ok(EnqueueOutcome::Pending(future))
      },
      | Err(error) => Err(error),
    }
  }

  /// Prepends user messages so they are processed before already queued user messages.
  ///
  /// When the underlying queue implements
  /// [`DequeMessageQueue`](super::deque_message_queue::DequeMessageQueue), each message is
  /// inserted at the front in O(1). Otherwise, the drain-and-requeue fallback is used.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, capacity checks fail, or queue restoration
  /// fails.
  pub(crate) fn prepend_user_messages(&self, messages: &VecDeque<AnyMessage>) -> Result<(), SendError> {
    let Some(first_message) = messages.front().cloned() else {
      return Ok(());
    };

    if self.is_suspended() {
      return Err(SendError::suspended(first_message));
    }

    let _guard = self.user_queue_lock.lock();

    let current_user_len = self.user.number_of_messages();
    if self.prepend_would_overflow(messages.len(), current_user_len) {
      return Err(SendError::full(first_message));
    }

    if let Some(deque) = self.user.as_deque() {
      return self.prepend_via_deque(deque, messages);
    }

    self.prepend_via_drain_and_requeue(messages, &first_message)
  }

  /// Efficient O(k) prepend path for deque-capable queues.
  fn prepend_via_deque(
    &self,
    deque: &dyn super::deque_message_queue::DequeMessageQueue,
    messages: &VecDeque<AnyMessage>,
  ) -> Result<(), SendError> {
    // Insert in reverse order so the first message in `messages` ends up at the front.
    for message in messages.iter().rev().cloned() {
      if let Err(error) = deque.enqueue_first(message) {
        self.publish_metrics_with_user_len(self.user.number_of_messages());
        return Err(error);
      }
    }
    self.publish_metrics_with_user_len(self.user.number_of_messages());
    Ok(())
  }

  /// Drain-and-requeue fallback for non-deque queues.
  fn prepend_via_drain_and_requeue(
    &self,
    messages: &VecDeque<AnyMessage>,
    first_message: &AnyMessage,
  ) -> Result<(), SendError> {
    let mut existing = VecDeque::new();
    while let Some(message) = self.user.dequeue() {
      existing.push_back(message);
    }

    let mut enqueue_result = Ok(());
    for message in messages.iter().cloned().chain(existing.iter().cloned()) {
      if let Err(error) = self.enqueue_for_prepend(message, first_message) {
        enqueue_result = Err(error);
        break;
      }
    }

    if let Err(_error) = enqueue_result {
      self.user.clean_up();
      let pid = self.pid();
      let system_state = self.system_state();
      let total_existing = existing.len();
      let mut restored: usize = 0;
      for message in existing {
        if self.enqueue_for_prepend(message.clone(), first_message).is_err() {
          // Route unrecoverable messages to dead letter storage
          if let Some(ref state) = system_state {
            state.record_dead_letter(message, DeadLetterReason::Dropped, pid);
          }
        } else {
          restored += 1;
        }
      }
      let lost = total_existing - restored;
      if lost > 0 {
        self.emit_log(
          LogLevel::Error,
          alloc::format!("mailbox prepend recovery: {lost} of {total_existing} message(s) routed to dead letters"),
        );
      }
      self.publish_metrics_with_user_len(self.user.number_of_messages());
      return Err(SendError::full(first_message.clone()));
    }

    self.publish_metrics_with_user_len(self.user.number_of_messages());
    Ok(())
  }

  /// Dequeues the next available message, prioritising system queue.
  #[must_use]
  pub(crate) fn dequeue(&self) -> Option<MailboxMessage> {
    if let Some(system) = self.system.pop() {
      self.publish_metrics();
      return Some(MailboxMessage::System(system));
    }

    if self.is_suspended() {
      return None;
    }

    let result = {
      let _guard = self.user_queue_lock.lock();
      self.user.dequeue().map(MailboxMessage::User)
    };
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
    let _guard = self.user_queue_lock.lock();
    self.user.number_of_messages()
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

  const fn prepend_would_overflow(&self, prepended_count: usize, current_user_len: usize) -> bool {
    let MailboxCapacity::Bounded { capacity } = self.policy.capacity() else {
      return false;
    };

    if matches!(self.policy.overflow(), MailboxOverflowStrategy::Grow) {
      return false;
    }

    current_user_len.saturating_add(prepended_count) > capacity.get()
  }

  fn enqueue_for_prepend(&self, message: AnyMessage, first_message: &AnyMessage) -> Result<(), SendError> {
    match self.user.enqueue(message) {
      | Ok(EnqueueOutcome::Enqueued) => Ok(()),
      | Ok(EnqueueOutcome::Pending(_)) => Err(SendError::full(first_message.clone())),
      | Err(error) => Err(error),
    }
  }

  fn publish_metrics(&self) {
    let user_len = {
      let _guard = self.user_queue_lock.lock();
      self.user.number_of_messages()
    };
    self.publish_metrics_with_user_len(user_len);
  }

  fn publish_metrics_with_user_len(&self, user_len: usize) {
    let guard = self.instrumentation.lock();
    if let Some(instrumentation) = guard.as_ref() {
      instrumentation.publish(user_len, self.system_len());
    }
  }
}
