//! Priority mailbox maintaining separate queues for system and user messages.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, string::String};
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::core::sync::{SharedAccess, WeakShared};
use spin::Once;

use super::{
  CloseRequestOutcome, DequeMessageQueue, MailboxScheduleState, RunFinishOutcome, ScheduleHints, SystemQueue,
  enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome, envelope::Envelope,
  mailbox_cleanup_policy::MailboxCleanupPolicy, mailbox_instrumentation::MailboxInstrumentation,
  message_queue::MessageQueue,
};
use crate::core::kernel::{
  actor::{
    ActorCell, Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::SendError,
    messaging::{AnyMessage, message_invoker::MessageInvokerShared, system_message::SystemMessage},
    props::{MailboxConfig, MailboxConfigError},
  },
  dispatch::mailbox::policy::MailboxPolicy,
  event::logging::LogLevel,
  system::{
    shared_factory::{MailboxLocked, MailboxSharedSet},
    state::SystemStateShared,
  },
};

/// Priority mailbox maintaining separate queues for system and user messages.
pub struct Mailbox {
  policy:          MailboxPolicy,
  system:          SystemQueue,
  user:            Box<dyn MessageQueue>,
  /// Compound-op lock (Pekko `putLock` equivalent).
  ///
  /// Protects multi-step operations that must be atomic with respect to
  /// `finalize_cleanup`: `enqueue_envelope_locked` (is_closed + enqueue),
  /// `prepend_user_messages_deque_locked` (is_closed + O(k) prepend), and
  /// `finalize_cleanup` itself (drain + clean_up + finish_cleanup).
  ///
  /// Single-step operations (dequeue, metrics reads) do **not** acquire
  /// this lock — the inner queue mutex provides sufficient synchronization.
  put_lock:        MailboxLocked<()>,
  state:           MailboxScheduleState,
  /// Write-once instrumentation hooks. Set once via
  /// [`set_instrumentation`](Self::set_instrumentation), read lock-free thereafter via
  /// `spin::Once::get()`.
  instrumentation: Once<MailboxInstrumentation>,
  cleanup_policy:  MailboxCleanupPolicy,
  /// Write-once message invoker. Set once via [`install_invoker`](Self::install_invoker),
  /// read lock-free thereafter via `spin::Once::get()`.
  invoker:         Once<MessageInvokerShared>,
  /// Write-once weak handle to the owning actor cell. Set by [`Mailbox::with_actor`]
  /// or [`install_actor`](Self::install_actor), read lock-free thereafter.
  actor:           Once<WeakShared<ActorCell>>,
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

  /// Creates a mailbox using the provided policy and explicit lock bundle.
  #[must_use]
  pub fn new_with_shared_set(policy: MailboxPolicy, shared_set: &MailboxSharedSet) -> Self {
    let queue = super::mailboxes::create_message_queue_from_policy(policy);
    Self::new_with_queue_and_shared_set(policy, queue, shared_set)
  }

  /// Creates a new mailbox from the provided configuration.
  ///
  /// When the config declares deque semantics and the policy is unbounded, this produces a
  /// deque-capable queue that supports O(1) front insertion in
  /// [`prepend_user_messages_deque`](Self::prepend_user_messages_deque).
  ///
  /// # Errors
  ///
  /// Returns [`MailboxConfigError`](crate::core::kernel::actor::props::MailboxConfigError) when the
  /// configuration contract is violated.
  pub fn new_from_config(config: &MailboxConfig) -> Result<Self, MailboxConfigError> {
    let shared_set = MailboxSharedSet::builtin();
    Self::new_from_config_with_shared_set(config, &shared_set)
  }

  /// Creates a mailbox from configuration using the supplied lock bundle.
  ///
  /// # Errors
  ///
  /// Returns [`MailboxConfigError`](crate::core::kernel::actor::props::MailboxConfigError) when the
  /// configuration contract is violated.
  pub fn new_from_config_with_shared_set(
    config: &MailboxConfig,
    shared_set: &MailboxSharedSet,
  ) -> Result<Self, MailboxConfigError> {
    let policy = config.policy();
    let queue = super::mailboxes::create_message_queue_from_config(config)?;
    Ok(Self::new_with_queue_and_shared_set(policy, queue, shared_set))
  }

  /// Creates a new mailbox using the provided policy and pre-built queue.
  #[must_use]
  pub(crate) fn new_with_queue(policy: MailboxPolicy, queue: Box<dyn MessageQueue>) -> Self {
    let shared_set = MailboxSharedSet::builtin();
    Self::new_with_queue_and_shared_set(policy, queue, &shared_set)
  }

  #[must_use]
  pub(crate) fn new_with_queue_and_shared_set(
    policy: MailboxPolicy,
    queue: Box<dyn MessageQueue>,
    shared_set: &MailboxSharedSet,
  ) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      put_lock: shared_set.put_lock(),
      state: MailboxScheduleState::new(),
      instrumentation: Once::new(),
      cleanup_policy: MailboxCleanupPolicy::DrainToDeadLetters,
      invoker: Once::new(),
      actor: Once::new(),
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
    let shared_set = MailboxSharedSet::builtin();
    Self::new_sharing_with_shared_set(policy, queue, &shared_set)
  }

  /// Creates a sharing mailbox using the supplied lock bundle.
  #[must_use]
  pub fn new_sharing_with_shared_set(
    policy: MailboxPolicy,
    queue: Box<dyn MessageQueue>,
    shared_set: &MailboxSharedSet,
  ) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      put_lock: shared_set.put_lock(),
      state: MailboxScheduleState::new(),
      instrumentation: Once::new(),
      cleanup_policy: MailboxCleanupPolicy::LeaveSharedQueue,
      invoker: Once::new(),
      actor: Once::new(),
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
    let shared_set = MailboxSharedSet::builtin();
    Self::with_actor_and_shared_set(actor, policy, queue, &shared_set)
  }

  /// Creates a mailbox bound to a specific actor using the supplied lock bundle.
  #[must_use]
  pub fn with_actor_and_shared_set(
    actor: WeakShared<ActorCell>,
    policy: MailboxPolicy,
    queue: Box<dyn MessageQueue>,
    shared_set: &MailboxSharedSet,
  ) -> Self {
    Self {
      policy,
      system: SystemQueue::new(),
      user: queue,
      put_lock: shared_set.put_lock(),
      state: MailboxScheduleState::new(),
      instrumentation: Once::new(),
      cleanup_policy: MailboxCleanupPolicy::DrainToDeadLetters,
      invoker: Once::new(),
      actor: {
        let once = Once::new();
        once.call_once(|| actor);
        once
      },
    }
  }

  /// Installs the weak actor handle (write-once).
  ///
  /// `ActorCell::create` calls this once the cell `ArcShared` is materialised
  /// so the legacy `Mailbox::new(policy)` constructor (which does not yet
  /// know the cell) can be late-bound to its owner.
  pub fn install_actor(&self, actor: WeakShared<ActorCell>) {
    self.actor.call_once(|| actor);
  }

  /// Returns a clone of the weak actor handle if one is installed.
  #[must_use]
  pub fn actor(&self) -> Option<WeakShared<ActorCell>> {
    self.actor.get().cloned()
  }

  /// Installs the message invoker that [`run`](Self::run) drives.
  ///
  /// Called from `ActorCell::create` so that the new dispatcher path can
  /// drain the mailbox without needing a back-reference to the dispatcher
  /// itself.
  pub fn install_invoker(&self, invoker: MessageInvokerShared) {
    self.invoker.call_once(|| invoker);
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
  pub fn run(&self, throughput: NonZeroUsize, _throughput_deadline: Option<Duration>) -> bool {
    if self.state.is_cleanup_done() {
      return false;
    }

    let close_requested_at_start = self.state.is_close_requested();
    let invoker = self.invoker.get().cloned();
    // invoker 不在で通常の drain path は早期終了してよい。ただし close が既に
    // 要求済みなら、これ以上 user/system delivery ができなくても terminal
    // cleanup を完了させるために run loop へ入る必要がある。
    if invoker.is_none() && !close_requested_at_start {
      return false;
    }

    // Phase 9.2: bail out if the owning actor cell has been dropped. The
    // weak handle is optional so legacy `Mailbox::new(policy)` callers (which
    // never installed an actor) keep their existing semantics.
    let actor_alive = self.actor.get().is_none_or(|weak| weak.upgrade().is_some());
    if !actor_alive && !close_requested_at_start {
      return false;
    }

    self.set_running();

    // Pekko `Mailbox.run()` 準拠（Mailbox.scala:228-238）:
    //   if (!isClosed) { processAllSystemMessages(); processMailbox() }
    //
    // close が既に要求されている場合は処理段階をスキップし、finish_run()
    // 以降の終端経路（FinalizeNow → finalize_cleanup）に直接進む。MB-H2 が
    // finalize_cleanup 内で残余 system/user を DL 送りするため、ここで追加
    // dequeue する必要はない（close_request_does_not_dequeue_additional_system_messages
    // が検証）。
    // invoker 不在かつ close 要求済みの場合も同様に処理段階をスキップし、
    // finish_run() → FinalizeNow を通して finalize_cleanup に到達させる。
    if !close_requested_at_start && let Some(ref invoker) = invoker {
      self.process_all_system_messages(invoker);
      self.process_mailbox(invoker, throughput);
    }
    // Deadline support is added in a follow-up change (MB-M1, Phase A3).
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
    match self.finish_run() {
      | RunFinishOutcome::Continue { pending_reschedule } => {
        let still_has_work = self.user_len() > 0 || self.system_len() > 0;
        pending_reschedule || still_has_work
      },
      | RunFinishOutcome::FinalizeNow => {
        self.finalize_cleanup();
        false
      },
      | RunFinishOutcome::Closed => false,
    }
  }

  /// Drains the entire system-message queue through the invoker, mirroring Pekko
  /// `processAllSystemMessages()` (Mailbox.scala:287-299).
  ///
  /// System messages are **unmetered** with respect to throughput: each call keeps
  /// popping until the queue is empty or a close request is observed mid-drain.
  /// Pekko relies on `actor.systemInvoke` for every message; fraktor-rs keeps
  /// `Suspend`/`Resume` mailbox-local so the schedule-state transitions do not
  /// round-trip through the invoker.
  ///
  /// The inner `is_close_requested` check on every iteration is load-bearing for
  /// the `close_request_does_not_dequeue_additional_system_messages` contract:
  /// once close is requested mid-drain, we stop issuing `systemInvoke` calls and
  /// let `finalize_cleanup` (MB-H2) redirect any remaining system messages to the
  /// dead-letter sink.
  fn process_all_system_messages(&self, invoker: &MessageInvokerShared) {
    while !self.state.is_close_requested() {
      let Some(message) = self.system.pop() else {
        break;
      };
      self.publish_metrics();
      match message {
        | SystemMessage::Suspend => self.suspend(),
        | SystemMessage::Resume => self.resume(),
        | other => {
          if let Err(error) = invoker.with_write(|i| i.system_invoke(other)) {
            self.emit_log(LogLevel::Error, alloc::format!("failed to invoke system message: {error:?}"));
          }
        },
      }
    }
  }

  /// Processes up to `throughput` user messages, interleaving a full system-message
  /// drain after every user invocation. Pekko mirror of `processMailbox()`
  /// (Mailbox.scala:261-278).
  ///
  /// The throughput counter is **user-only**: system-message drains inside this
  /// loop do not decrement it. Any `Suspend` arriving via `process_all_system_messages`
  /// between two user messages flips `should_process_message` to `false` on the
  /// next iteration, gating the remaining user messages until a later `run()` call
  /// observes a `Resume` (AC-H1-T2 / T5).
  fn process_mailbox(&self, invoker: &MessageInvokerShared, throughput: NonZeroUsize) {
    let mut left = throughput.get();
    while left > 0 && self.should_process_message() {
      let Some(envelope) = self.dequeue() else {
        break;
      };
      let payload = envelope.into_payload();
      if let Err(error) = invoker.with_write(|i| i.invoke(payload)) {
        self.emit_log(LogLevel::Error, alloc::format!("failed to invoke user message: {error:?}"));
      }
      // Pekko: `actor.invoke(next); processAllSystemMessages()` — each user
      // message is followed by a full system drain so Suspend / Resume /
      // Stop arriving mid-run are reflected before the next user message.
      self.process_all_system_messages(invoker);
      left -= 1;
    }
  }

  /// Pekko `shouldProcessMessage` (Mailbox.scala:126) — true while the mailbox
  /// can legitimately process another user message.
  ///
  /// `cleanup_done` is included so we bail out immediately if `finish_run()` has
  /// already transitioned the state machine (defensive; not expected during a
  /// single `run()`).
  fn should_process_message(&self) -> bool {
    !self.is_suspended() && !self.state.is_close_requested() && !self.state.is_cleanup_done()
  }

  /// Pops the next user envelope, mirroring Pekko `Mailbox.dequeue()`
  /// (Mailbox.scala:115). Rejects the pop when the mailbox is closed or
  /// suspended, and publishes metrics on a successful pop so observers see
  /// the queue-length drop immediately.
  ///
  /// The closed / suspended guards intentionally duplicate the check performed
  /// by `should_process_message` in the caller's loop condition. This is a
  /// TOCTOU guard: `Suspend` / `Close` can be delivered by another thread
  /// between the `should_process_message()` evaluation and the `self.user.dequeue()`
  /// call, so re-checking here ensures no user message is consumed after a
  /// state transition that the caller already decided to honour. Do not remove
  /// the duplication even though it reads as redundant on the single-threaded
  /// happy path.
  pub(crate) fn dequeue(&self) -> Option<Envelope> {
    if self.state.is_close_requested() || self.is_suspended() {
      return None;
    }
    let result = self.user.dequeue();
    if result.is_some() {
      self.publish_metrics();
    }
    result
  }

  /// Returns the cleanup policy configured for this mailbox.
  #[must_use]
  pub const fn cleanup_policy(&self) -> MailboxCleanupPolicy {
    self.cleanup_policy
  }

  /// Installs instrumentation hooks for metrics emission.
  pub(crate) fn set_instrumentation(&self, instrumentation: MailboxInstrumentation) {
    self.instrumentation.call_once(|| instrumentation);
  }

  /// Returns the system state handle if instrumentation has been installed.
  pub(crate) fn system_state(&self) -> Option<SystemStateShared> {
    self.instrumentation.get().and_then(|inst| inst.system_state())
  }

  /// Returns the actor pid associated with this mailbox when instrumentation is installed.
  #[must_use]
  pub(crate) fn pid(&self) -> Option<Pid> {
    self.instrumentation.get().map(|inst| inst.pid())
  }

  /// Emits a log message tagged with this mailbox pid.
  pub(crate) fn emit_log(&self, level: LogLevel, message: impl Into<String>) {
    if let Some(instrumentation) = self.instrumentation.get() {
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

  /// Convenience wrapper around [`Self::enqueue_envelope`] that wraps a bare
  /// [`AnyMessage`] into an [`Envelope`]. Used by tests, benchmarks, and
  /// stash paths that do not own an envelope; dispatcher-side code should
  /// call [`Self::enqueue_envelope`] directly to preserve sender metadata.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is closed or the underlying queue
  /// rejects the envelope (e.g. bounded overflow).
  #[cfg_attr(not(test), doc(hidden))]
  pub fn enqueue_user(&self, message: AnyMessage) -> Result<(), SendError> {
    self.enqueue_envelope(Envelope::new(message))
  }

  /// Enqueues an envelope into the user queue.
  ///
  /// This is the dispatcher-side dispatch path used by the
  /// `MessageDispatcher` family.
  ///
  /// Pekko parity: suspension only blocks **dequeue**; enqueue always
  /// accepts the envelope so that suspended actors still buffer inbound
  /// messages and observe them once resumed.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is closed or the underlying queue
  /// rejects the envelope (e.g. bounded overflow).
  pub fn enqueue_envelope(&self, envelope: Envelope) -> Result<(), SendError> {
    // Fast path: closed mailboxes are terminal and reject enqueues.
    // Suspension is intentionally NOT checked here — Pekko's contract keeps
    // the enqueue path open while suspended and only gates dequeue.
    if self.is_closed() {
      return Err(SendError::closed(envelope.into_payload()));
    }
    self.enqueue_envelope_locked(envelope)
  }

  /// Locked critical section of [`Self::enqueue_envelope`].
  ///
  /// Acquires `put_lock` and performs the authoritative close
  /// re-check before handing the envelope to the underlying queue. This
  /// must only be called from [`Self::enqueue_envelope`] in production
  /// code; the fast path preceding this method is what makes the common
  /// closed / suspended paths lock-free.
  fn enqueue_envelope_locked(&self, envelope: Envelope) -> Result<(), SendError> {
    let enqueue_result = self.put_lock.with_lock(|_| {
      // Authoritative re-check under lock: cleanup may have won the lock
      // race between the fast path and this acquisition. Without this, a
      // producer could phantom-enqueue into a drained queue.
      if self.is_closed() {
        return Err(EnqueueError::new(SendError::closed(envelope.into_payload())));
      }
      self.user.enqueue(envelope)
    });

    match enqueue_result {
      | Ok(EnqueueOutcome::Accepted) => {
        self.publish_metrics();
        Ok(())
      },
      | Ok(EnqueueOutcome::Evicted(evicted)) => {
        // Pekko 互換: DropOldest で押し出された envelope を MailboxFull として
        // DeadLetter に通知する。エンキュー自体は成功したので呼び出し元には
        // Ok(()) を返す（Pekko `BoundedNodeMessageQueue.enqueue` 相当）。
        if let Some(state) = self.system_state() {
          state.record_dead_letter(evicted.into_payload(), DeadLetterReason::MailboxFull, self.pid());
        }
        self.publish_metrics();
        Ok(())
      },
      | Ok(EnqueueOutcome::Rejected(rejected)) => {
        // Pekko 互換: DropNewest で拒否された envelope を MailboxFull として
        // DeadLetter に通知する。mailbox 層が唯一の DL 記録源となり、上流は
        // 成功 (Ok(())) を観測する (Pekko `BoundedMailbox.enqueue` / `BoundedNodeMessageQueue.enqueue`
        // の void 返却 + 内部 deadLetters 転送と等価)。
        if let Some(state) = self.system_state() {
          state.record_dead_letter(rejected.into_payload(), DeadLetterReason::MailboxFull, self.pid());
        }
        self.publish_metrics();
        Ok(())
      },
      | Err(enqueue_error) => {
        let (send_error, evicted) = enqueue_error.into_parts();
        // 病的ケースで DropOldest が eviction を発行した直後に offer が失敗した
        // 場合、evicted をロストさせず DeadLetter へ転送する。
        if let (Some(evicted_envelope), Some(state)) = (evicted, self.system_state()) {
          state.record_dead_letter(evicted_envelope.into_payload(), DeadLetterReason::MailboxFull, self.pid());
        }
        // DropNewest/DropOldest オーバーフローは `Ok(Rejected|Evicted)` として
        // 成功扱いされるため、ここに到達するのは真の失敗 (Closed / Timeout /
        // Suspended / NoRecipient / InvalidPayload / backend alloc failure) のみ。
        // これらは mailbox 層では DL 記録せず、上流で `record_send_error` に
        // 委ねる (他に DL 記録源がないため)。
        Err(send_error)
      },
    }
  }

  /// Returns the deque capability of the user queue when available.
  #[must_use]
  pub(crate) fn user_deque(&self) -> Option<&dyn DequeMessageQueue> {
    self.user.as_deque()
  }

  /// Prepends user messages so they are processed before already queued user messages.
  ///
  /// Callers must first resolve deque capability via [`Self::user_deque`]. The resolved
  /// capability is passed back into the locked prepend path so the lock responsibility remains
  /// with `Mailbox`.
  ///
  /// Pekko parity: suspension only blocks dequeue; prepends are accepted
  /// while suspended so the buffered order is preserved until resume.
  ///
  /// # Errors
  ///
  /// Returns an error if the mailbox is closed or the underlying deque rejects the
  /// prepend.
  pub(crate) fn prepend_user_messages_deque(
    &self,
    resolved_deque: &dyn DequeMessageQueue,
    messages: &VecDeque<AnyMessage>,
  ) -> Result<(), SendError> {
    let Some(first_message) = messages.front().cloned() else {
      return Ok(());
    };

    // Fast path: closed mailboxes reject prepends. Suspension is intentionally
    // NOT checked here — enqueue / prepend accept while suspended and only
    // dequeue is gated.
    if self.is_closed() {
      return Err(SendError::closed(first_message));
    }
    self.prepend_user_messages_deque_locked(resolved_deque, messages, first_message)
  }

  /// Locked critical section of [`Self::prepend_user_messages_deque`].
  ///
  /// Acquires `put_lock`, performs the authoritative close re-check,
  /// and prepends in O(k). Must only be called
  /// from [`Self::prepend_user_messages_deque`] after the fast path has cleared.
  fn prepend_user_messages_deque_locked(
    &self,
    deque: &dyn DequeMessageQueue,
    messages: &VecDeque<AnyMessage>,
    first_message: AnyMessage,
  ) -> Result<(), SendError> {
    self.put_lock.with_lock(|_| {
      // Authoritative re-check under lock: cleanup may have won the lock race
      // between the fast path and this acquisition.
      if self.is_closed() {
        return Err(SendError::closed(first_message));
      }

      self.prepend_via_deque(deque, messages)
    })
  }

  /// Efficient O(k) prepend path for deque-capable queues.
  fn prepend_via_deque(&self, deque: &dyn DequeMessageQueue, messages: &VecDeque<AnyMessage>) -> Result<(), SendError> {
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

  /// Dequeues the next available message, prioritising system queue.
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

  /// Clears scheduled/running flags and returns whether a pending reschedule must occur
  /// immediately.
  #[must_use]
  pub(crate) fn set_idle(&self) -> bool {
    self.state.set_idle()
  }

  #[must_use]
  pub(crate) fn finish_run(&self) -> RunFinishOutcome {
    self.state.finish_run()
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
  /// Pekko-style alias mirroring `Mailbox.canBeScheduledForExecution`
  /// (Mailbox.scala:148-155). The scheduling gate follows the same
  /// status-dependent rule as Pekko:
  ///
  /// - `Closed`: never schedulable.
  /// - `Open` / `Scheduled` (not suspended): schedulable whenever either queue has work or the
  ///   caller provides a hint.
  /// - Suspended: schedulable only when system work exists (the hint or actual pending system
  ///   messages), because system messages such as `Resume`, `Terminate`, `Watch`, and failure
  ///   handling must be delivered even while the user side is suspended. This is required after
  ///   MB-H1: since `enqueue_envelope` now accepts user messages while suspended, we must still be
  ///   able to schedule the mailbox to drain the system queue (e.g. process `Resume`) so the newly
  ///   accepted user messages can be drained once resumed.
  #[must_use]
  pub fn can_be_scheduled_for_execution(&self, hints: ScheduleHints) -> bool {
    if self.is_closed() {
      return false;
    }
    if self.is_suspended() {
      return hints.has_system_messages || self.system_len() > 0;
    }
    hints.has_system_messages || hints.has_user_messages || self.system_len() > 0 || self.user_len() > 0
  }

  /// Transitions the mailbox to the closed terminal state, drains the user
  /// queue (subject to the mailbox's [`MailboxCleanupPolicy`]), and routes
  /// any remaining envelopes to the dead-letter destination when the policy
  /// is [`MailboxCleanupPolicy::DrainToDeadLetters`].
  ///
  /// Mirrors Pekko `Mailbox.scala:178` `becomeClosed()` combined with
  /// `cleanUp()` as an atomic operation: the state transition and queue drain
  /// are composed under `CloseRequestOutcome` so the finalizer runs exactly
  /// once. Called from `MessageDispatcherShared::detach` during cell teardown
  /// to ensure no further executions can be scheduled and in-flight envelopes
  /// are observed exactly once.
  pub fn become_closed(&self) {
    match self.state.request_close() {
      | CloseRequestOutcome::CallerOwnsFinalizer => self.finalize_cleanup(),
      | CloseRequestOutcome::RunnerOwnsFinalizer
      | CloseRequestOutcome::AlreadyRequested
      | CloseRequestOutcome::AlreadyCleaned => {},
    }
  }

  fn finalize_cleanup(&self) {
    let pid = self.pid();
    let system_state = self.system_state();
    let user_len_after_cleanup = self.put_lock.with_lock(|_| {
      // MB-H2 (Pekko parity): `Mailbox.cleanUp()` は user queue のクリーンアップ方針とは無関係に
      // system queue を必ず DeadLetters へ drain する。各 mailbox は自身専用の `SystemQueue` を
      // 所有しており共有されないため、`LeaveSharedQueue` であっても pending な
      // `Watch` / `Terminated` / `Create` / `Stop` envelope を失って観測不能にしてはならない。
      while let Some(sys_msg) = self.system.pop() {
        if let Some(ref state) = system_state {
          state.record_dead_letter(AnyMessage::new(sys_msg), DeadLetterReason::Dropped, pid);
        }
      }
      // user queue の扱いのみ cleanup policy に従う。`DrainToDeadLetters` なら残留 user message を
      // DeadLetters へ転送、`LeaveSharedQueue` なら共有 queue 側に委ねる。
      if matches!(self.cleanup_policy, MailboxCleanupPolicy::DrainToDeadLetters) {
        while let Some(envelope) = self.user.dequeue() {
          if let Some(ref state) = system_state {
            state.record_dead_letter(envelope.into_payload(), DeadLetterReason::Dropped, pid);
          }
        }
      }
      self.user.clean_up();
      let user_len = self.user.number_of_messages();
      self.state.finish_cleanup();
      user_len
    });
    self.publish_metrics_with_user_len(user_len_after_cleanup);
  }

  /// Returns whether the mailbox is in the terminal closed state.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    self.state.is_closed()
  }

  /// Indicates whether the mailbox is currently suspended.
  #[must_use]
  pub fn is_suspended(&self) -> bool {
    self.state.is_suspended()
  }

  /// Returns `true` while the drain loop is actively running.
  ///
  /// Used by routing logic to approximate Pekko's `isProcessingMessage` — a
  /// running mailbox indicates the actor is currently handling a message.
  #[must_use]
  pub fn is_running(&self) -> bool {
    self.state.is_running()
  }

  /// Returns the number of user messages awaiting processing.
  #[must_use]
  pub(crate) fn user_len(&self) -> usize {
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

  fn publish_metrics(&self) {
    self.publish_metrics_with_user_len(self.user.number_of_messages());
  }

  fn publish_metrics_with_user_len(&self, user_len: usize) {
    if let Some(instrumentation) = self.instrumentation.get() {
      instrumentation.publish(user_len, self.system_len());
    }
  }
}
