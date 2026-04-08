//! Priority mailbox maintaining separate queues for system and user messages.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, string::String};
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess, WeakShared};

use super::{
  MailboxScheduleState, ScheduleHints, SystemQueue, envelope::Envelope, mailbox_cleanup_policy::MailboxCleanupPolicy,
  mailbox_instrumentation::MailboxInstrumentation, mailbox_message::MailboxMessage, message_queue::MessageQueue,
};
use crate::core::kernel::{
  actor::{
    ActorCell, Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::SendError,
    messaging::{AnyMessage, message_invoker::MessageInvokerShared, system_message::SystemMessage},
    props::{MailboxConfig, MailboxConfigError},
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
  cleanup_policy:  MailboxCleanupPolicy,
  invoker:         ArcShared<RuntimeMutex<Option<MessageInvokerShared>>>,
  /// Weak handle to the owning actor cell. Set by [`Mailbox::with_actor`] and
  /// observed by [`Mailbox::run`] so the drain loop can early-return when the
  /// cell has been dropped.
  actor:           ArcShared<RuntimeMutex<Option<WeakShared<ActorCell>>>>,
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
  pub fn new_from_config(config: &MailboxConfig) -> Result<Self, MailboxConfigError> {
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
      cleanup_policy: MailboxCleanupPolicy::DrainToDeadLetters,
      invoker: ArcShared::new(RuntimeMutex::new(None)),
      actor: ArcShared::new(RuntimeMutex::new(None)),
    }
  }

  /// Creates a sharing mailbox that delegates to a shared message queue.
  ///
  /// The resulting mailbox is configured with
  /// [`MailboxCleanupPolicy::LeaveSharedQueue`], so its `clean_up` does not
  /// drain the underlying queue. This is the constructor used by
  /// `BalancingDispatcher::create_mailbox`.
  #[must_use]
  pub fn new_sharing(policy: MailboxPolicy, queue: Box<dyn MessageQueue>) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      user_queue_lock: ArcShared::new(RuntimeMutex::new(())),
      state: MailboxScheduleState::new(),
      instrumentation: ArcShared::new(RuntimeMutex::new(None)),
      cleanup_policy: MailboxCleanupPolicy::LeaveSharedQueue,
      invoker: ArcShared::new(RuntimeMutex::new(None)),
      actor: ArcShared::new(RuntimeMutex::new(None)),
    }
  }

  /// Creates a mailbox bound to a specific actor cell.
  ///
  /// This is the canonical Pekko-aligned constructor: the mailbox captures a
  /// weak reference to the owning [`ActorCell`] so the drain loop in
  /// [`Mailbox::run`] can early-return after the cell has been dropped, and
  /// so detach paths can transition the mailbox to a terminal state and run
  /// `clean_up` without needing the dispatcher to thread the reference back
  /// through.
  ///
  /// `queue` may be a freshly-built per-actor queue or a clone of a shared
  /// queue (used by `BalancingDispatcher::create_mailbox`).
  #[must_use]
  pub fn with_actor(actor: WeakShared<ActorCell>, policy: MailboxPolicy, queue: Box<dyn MessageQueue>) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      user_queue_lock: ArcShared::new(RuntimeMutex::new(())),
      state: MailboxScheduleState::new(),
      instrumentation: ArcShared::new(RuntimeMutex::new(None)),
      cleanup_policy: MailboxCleanupPolicy::DrainToDeadLetters,
      invoker: ArcShared::new(RuntimeMutex::new(None)),
      actor: ArcShared::new(RuntimeMutex::new(Some(actor))),
    }
  }

  /// Replaces the weak actor handle, returning the previous one.
  ///
  /// `ActorCell::create` calls this once the cell `ArcShared` is materialised
  /// so the legacy `Mailbox::new(policy)` constructor (which does not yet
  /// know the cell) can be late-bound to its owner.
  pub fn install_actor(&self, actor: WeakShared<ActorCell>) -> Option<WeakShared<ActorCell>> {
    self.actor.lock().replace(actor)
  }

  /// Returns a clone of the weak actor handle if one is installed.
  #[must_use]
  pub fn actor(&self) -> Option<WeakShared<ActorCell>> {
    self.actor.lock().clone()
  }

  /// Installs the message invoker that [`run`](Self::run) drives.
  ///
  /// Called from `ActorCell::create` so that the new dispatcher path can
  /// drain the mailbox without needing a back-reference to the dispatcher
  /// itself.
  pub fn install_invoker(&self, invoker: MessageInvokerShared) {
    *self.invoker.lock() = Some(invoker);
  }

  /// Drains the mailbox up to `throughput` messages, invoking each one through the installed
  /// invoker.
  ///
  /// This is the entry point used by the new `MessageDispatcherShared::register_for_execution`
  /// closure: the dispatcher submits a closure that calls `mailbox.run(throughput, deadline)` and
  /// the mailbox itself owns the drain loop.
  ///
  /// # Returns
  ///
  /// Returns `true` when the mailbox state machine reports a pending
  /// reschedule (i.e. additional work arrived while the drain was in
  /// progress). The dispatcher closure must observe this signal and call
  /// `register_for_execution` again, otherwise the late-arriving messages
  /// would sit in the queue without anyone to wake the mailbox up — the
  /// `tell()` paths that delivered them already saw the mailbox in the
  /// `running` state and returned without scheduling.
  ///
  /// Returns `false` when no invoker / actor cell is available (no-op
  /// fallback for legacy `Mailbox::new(policy)` callers that never
  /// installed an actor) and when the drain finishes cleanly with no
  /// pending reschedule.
  #[must_use]
  pub fn run(&self, throughput: NonZeroUsize, throughput_deadline: Option<Duration>) -> bool {
    let invoker = self.invoker.lock().clone();
    let Some(invoker) = invoker else {
      return false;
    };

    // Phase 9.2: bail out if the owning actor cell has been dropped. The
    // weak handle is optional so legacy `Mailbox::new(policy)` callers (which
    // never installed an actor) keep their existing semantics.
    let actor_alive = match self.actor.lock().as_ref() {
      | Some(weak) => weak.upgrade().is_some(),
      | None => true,
    };
    if !actor_alive {
      return false;
    }

    self.set_running();
    let mut processed: usize = 0;
    let limit = throughput.get();
    let _ = throughput_deadline; // Deadline support is added in a follow-up change.

    while processed < limit {
      match self.dequeue() {
        | Some(MailboxMessage::System(msg)) => {
          // Suspend / Resume are mailbox-local commands; everything else delegates to the invoker.
          match msg {
            | SystemMessage::Suspend => self.suspend(),
            | SystemMessage::Resume => self.resume(),
            | other => {
              if let Err(error) = invoker.with_write(|i| i.invoke_system_message(other)) {
                self.emit_log(LogLevel::Error, alloc::format!("failed to invoke system message: {error:?}"));
              }
            },
          }
          processed += 1;
        },
        | Some(MailboxMessage::User(envelope)) => {
          let payload = envelope.into_payload();
          if let Err(error) = invoker.with_write(|i| i.invoke_user_message(payload)) {
            self.emit_log(LogLevel::Error, alloc::format!("failed to invoke user message: {error:?}"));
          }
          processed += 1;
        },
        | None => break,
      }
    }
    // Surface the "needs reschedule" signal to the caller. The signal is
    // a union of two independent sources:
    //
    // 1. **Producer signal** (`need_reschedule`, consumed by `set_idle`): `request_schedule` sets this
    //    flag when a `tell()` arrives while the mailbox is busy. Without `set_idle`'s return value the
    //    dispatcher would never know that work arrived during the drain.
    //
    // 2. **Consumer signal** (queue still has messages after the drain): The throughput limit is a
    //    yield point, not a "queue is empty" signal. When we hit the limit (or even when the limit was
    //    not reached but envelopes were already in the queue before we started, e.g. if a
    //    `BalancingDispatcher` team queue holds messages enqueued by tells that scheduled a different
    //    team member), the queue can still have pending work that no producer will ever announce again
    //    — the producers may have already finished firing all their tells. Self-reporting via the queue
    //    state is the only way to keep the drain loop alive in that case.
    //
    // The dispatcher closure that wraps `run()` must re-call
    // `register_for_execution` whenever this combined signal is true,
    // otherwise late-arriving or already-queued messages would sit in
    // the mailbox without anyone to wake it up.
    let pending_reschedule = self.set_idle();
    let still_has_work = self.user_len() > 0 || self.system_len() > 0;
    pending_reschedule || still_has_work
  }

  /// Returns the cleanup policy configured for this mailbox.
  #[must_use]
  pub const fn cleanup_policy(&self) -> MailboxCleanupPolicy {
    self.cleanup_policy
  }

  /// Installs instrumentation hooks for metrics emission.
  pub(crate) fn set_instrumentation(&self, instrumentation: MailboxInstrumentation) {
    *self.instrumentation.lock() = Some(instrumentation);
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
  /// This is the legacy convenience entry point used by tests and stash paths
  /// that have an `AnyMessage` in hand. The message is wrapped in an
  /// [`Envelope`] before being handed to the underlying queue.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, full, or closed.
  #[cfg_attr(not(test), doc(hidden))]
  pub fn enqueue_user(&self, message: AnyMessage) -> Result<(), SendError> {
    self.enqueue_envelope(Envelope::new(message))
  }

  /// Enqueues an envelope into the user queue.
  ///
  /// This is the dispatcher-side dispatch path used by the
  /// `MessageDispatcher` family.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is suspended, full, or closed.
  pub fn enqueue_envelope(&self, envelope: Envelope) -> Result<(), SendError> {
    // Fast path: reject closed before suspended. `Closed` is terminal, so a
    // mailbox observed as both closed and suspended MUST return `Closed`.
    if self.is_closed() {
      return Err(SendError::closed(envelope.into_payload()));
    }
    if self.is_suspended() {
      return Err(SendError::suspended(envelope.into_payload()));
    }
    self.enqueue_envelope_locked(envelope)
  }

  /// Locked critical section of [`Self::enqueue_envelope`].
  ///
  /// Acquires `user_queue_lock` and performs the authoritative close
  /// re-check before handing the envelope to the underlying queue. This
  /// must only be called from [`Self::enqueue_envelope`] in production
  /// code; the fast path preceding this method is what makes the common
  /// closed / suspended paths lock-free.
  fn enqueue_envelope_locked(&self, envelope: Envelope) -> Result<(), SendError> {
    let enqueue_result = {
      let _guard = self.user_queue_lock.lock();
      // Authoritative re-check under lock: cleanup may have won the lock
      // race between the fast path and this acquisition. Without this, a
      // producer could phantom-enqueue into a drained queue.
      if self.is_closed() {
        return Err(SendError::closed(envelope.into_payload()));
      }
      self.user.enqueue(envelope)
    };

    match enqueue_result {
      | Ok(()) => {
        self.publish_metrics();
        Ok(())
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

    // Fast path: reject closed before suspended (same rationale as
    // `enqueue_envelope`).
    if self.is_closed() {
      return Err(SendError::closed(first_message));
    }
    if self.is_suspended() {
      return Err(SendError::suspended(first_message));
    }
    self.prepend_user_messages_locked(messages, first_message)
  }

  /// Locked critical section of [`Self::prepend_user_messages`].
  ///
  /// Acquires `user_queue_lock`, performs the authoritative close re-check,
  /// runs the capacity check, and dispatches to the deque or drain-and-
  /// requeue path. Must only be called from
  /// [`Self::prepend_user_messages`] in production code after the fast
  /// path has cleared.
  fn prepend_user_messages_locked(
    &self,
    messages: &VecDeque<AnyMessage>,
    first_message: AnyMessage,
  ) -> Result<(), SendError> {
    let _guard = self.user_queue_lock.lock();

    // Authoritative re-check under lock: cleanup may have won the lock race
    // between the fast path and this acquisition.
    if self.is_closed() {
      return Err(SendError::closed(first_message));
    }

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
      if let Err(error) = deque.enqueue_first(Envelope::new(message)) {
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
    let mut existing: VecDeque<Envelope> = VecDeque::new();
    while let Some(envelope) = self.user.dequeue() {
      existing.push_back(envelope);
    }

    let mut enqueue_result = Ok(());
    let new_envelopes = messages.iter().cloned().map(Envelope::new);
    let existing_envelopes = existing.iter().cloned();
    for envelope in new_envelopes.chain(existing_envelopes) {
      if let Err(error) = self.user.enqueue(envelope) {
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
      for envelope in existing {
        if self.user.enqueue(envelope.clone()).is_err() {
          // Route unrecoverable messages to dead letter storage
          if let Some(ref state) = system_state {
            state.record_dead_letter(envelope.into_payload(), DeadLetterReason::Dropped, pid);
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

  /// Pekko-style alias for [`request_schedule`](Self::request_schedule).
  ///
  /// Used by the new `dispatcher` module's `register_for_execution`
  /// orchestration to attempt the CAS that transitions the mailbox from idle
  /// to scheduled.
  #[must_use]
  pub fn set_as_scheduled(&self, hints: ScheduleHints) -> bool {
    self.state.request_schedule(hints)
  }

  /// Pekko-style alias for [`set_idle`](Self::set_idle).
  pub fn set_as_idle(&self) -> bool {
    self.state.set_idle()
  }

  /// Returns `true` when the mailbox is currently eligible for scheduling.
  ///
  /// Pekko-style alias mirroring `Mailbox.canBeScheduledForExecution`.
  #[must_use]
  pub fn can_be_scheduled_for_execution(&self, _hints: ScheduleHints) -> bool {
    !self.is_closed() && !self.is_suspended()
  }

  /// Transitions the mailbox to the closed terminal state, drains the user
  /// queue (subject to the mailbox's [`MailboxCleanupPolicy`]), and routes
  /// any remaining envelopes to the dead-letter destination when the policy
  /// is [`MailboxCleanupPolicy::DrainToDeadLetters`].
  ///
  /// Called from `MessageDispatcherShared::detach` so the dispatcher detach
  /// path mirrors Pekko's `Mailbox.becomeClosed` + `cleanUp` contract: once
  /// the cell is being torn down, no further executions can be scheduled and
  /// in-flight envelopes are observed exactly once.
  pub fn become_closed_and_clean_up(&self) {
    let pid = self.pid();
    let system_state = self.system_state();
    // Acquire `user_queue_lock` before `state.close()` so that any in-flight
    // `enqueue_envelope` / `prepend_user_messages` waiting on the lock will
    // observe `is_closed() == true` in their under-lock re-check and reject
    // instead of mutating a drained queue. This serialization boundary is
    // maintained regardless of `cleanup_policy` to also protect direct
    // mailbox APIs on sharing mailboxes.
    let user_len_after_cleanup = {
      let _guard = self.user_queue_lock.lock();
      self.state.close();
      if matches!(self.cleanup_policy, MailboxCleanupPolicy::DrainToDeadLetters) {
        while let Some(envelope) = self.user.dequeue() {
          if let Some(ref state) = system_state {
            state.record_dead_letter(envelope.into_payload(), DeadLetterReason::Dropped, pid);
          }
        }
      }
      self.user.clean_up();
      self.user.number_of_messages()
    };
    self.publish_metrics_with_user_len(user_len_after_cleanup);
  }

  /// Returns whether the mailbox is in the terminal closed state.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.state.is_closed()
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
