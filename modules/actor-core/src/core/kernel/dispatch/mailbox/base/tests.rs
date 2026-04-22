use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{
  Arc,
  mpsc::{Receiver, Sender},
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::{ActorError, SendError},
    messaging::{
      AnyMessage,
      message_invoker::{MessageInvoker, MessageInvokerShared},
      system_message::SystemMessage,
    },
  },
  dispatch::mailbox::{
    CloseRequestOutcome, DequeMessageQueue, EnqueueError, EnqueueOutcome, Envelope, Mailbox, MailboxInstrumentation,
    MailboxOverflowStrategy, MailboxPolicy, MessageQueue, ScheduleHints, UnboundedDequeMessageQueue,
  },
  system::ActorSystem,
};

enum ScriptedEnqueue {
  Enqueued,
  Full,
  Closed,
}

struct ScriptedMessageQueue {
  messages:  SharedLock<VecDeque<Envelope>>,
  outcomes:  SharedLock<VecDeque<ScriptedEnqueue>>,
  full_hook: SharedLock<Option<ScriptedFullHook>>,
}

struct ScriptedFullHook {
  before_error_tx: Sender<()>,
  resume_rx:       Receiver<()>,
}

impl ScriptedFullHook {
  fn new(before_error_tx: Sender<()>, resume_rx: Receiver<()>) -> Self {
    Self { before_error_tx, resume_rx }
  }
}

impl ScriptedMessageQueue {
  fn new(outcomes: VecDeque<ScriptedEnqueue>) -> Self {
    Self::new_with_full_hook(outcomes, None)
  }

  fn new_with_full_hook(outcomes: VecDeque<ScriptedEnqueue>, full_hook: Option<ScriptedFullHook>) -> Self {
    Self {
      messages:  SharedLock::new_with_driver::<SpinSyncMutex<_>>(VecDeque::new()),
      outcomes:  SharedLock::new_with_driver::<SpinSyncMutex<_>>(outcomes),
      full_hook: SharedLock::new_with_driver::<SpinSyncMutex<_>>(full_hook),
    }
  }
}

impl MessageQueue for ScriptedMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    let outcome = self.outcomes.with_lock(|outcomes| outcomes.pop_front()).expect("enqueue outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.with_lock(|messages| messages.push_back(envelope));
        Ok(EnqueueOutcome::Accepted)
      },
      | ScriptedEnqueue::Full => {
        if let Some(hook) = self.full_hook.with_lock(|full_hook| full_hook.take()) {
          hook.before_error_tx.send(()).expect("full hook notification must be delivered");
          hook.resume_rx.recv().expect("full hook resume signal must be delivered");
        }
        Err(EnqueueError::new(SendError::full(envelope.into_payload())))
      },
      | ScriptedEnqueue::Closed => Err(EnqueueError::new(SendError::closed(envelope.into_payload()))),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.messages.with_lock(|messages| messages.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.messages.with_lock(|messages| messages.len())
  }

  fn clean_up(&self) {
    self.messages.with_lock(|messages| messages.clear());
  }
}

struct ScriptedDequeMessageQueue {
  messages:                SharedLock<VecDeque<Envelope>>,
  enqueue_outcomes:        SharedLock<VecDeque<ScriptedEnqueue>>,
  enqueue_first_outcomes:  SharedLock<VecDeque<ScriptedEnqueue>>,
  enqueue_first_full_hook: SharedLock<Option<ScriptedFullHook>>,
}

impl ScriptedDequeMessageQueue {
  fn new(enqueue_outcomes: VecDeque<ScriptedEnqueue>, enqueue_first_outcomes: VecDeque<ScriptedEnqueue>) -> Self {
    Self::new_with_enqueue_first_hook(enqueue_outcomes, enqueue_first_outcomes, None)
  }

  fn new_with_enqueue_first_hook(
    enqueue_outcomes: VecDeque<ScriptedEnqueue>,
    enqueue_first_outcomes: VecDeque<ScriptedEnqueue>,
    enqueue_first_full_hook: Option<ScriptedFullHook>,
  ) -> Self {
    Self {
      messages:                SharedLock::new_with_driver::<SpinSyncMutex<_>>(VecDeque::new()),
      enqueue_outcomes:        SharedLock::new_with_driver::<SpinSyncMutex<_>>(enqueue_outcomes),
      enqueue_first_outcomes:  SharedLock::new_with_driver::<SpinSyncMutex<_>>(enqueue_first_outcomes),
      enqueue_first_full_hook: SharedLock::new_with_driver::<SpinSyncMutex<_>>(enqueue_first_full_hook),
    }
  }
}

impl MessageQueue for ScriptedDequeMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    let outcome = self
      .enqueue_outcomes
      .with_lock(|enqueue_outcomes| enqueue_outcomes.pop_front())
      .expect("enqueue outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.with_lock(|messages| messages.push_back(envelope));
        Ok(EnqueueOutcome::Accepted)
      },
      | ScriptedEnqueue::Full => Err(EnqueueError::new(SendError::full(envelope.into_payload()))),
      | ScriptedEnqueue::Closed => Err(EnqueueError::new(SendError::closed(envelope.into_payload()))),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.messages.with_lock(|messages| messages.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.messages.with_lock(|messages| messages.len())
  }

  fn clean_up(&self) {
    self.messages.with_lock(|messages| messages.clear());
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    Some(self)
  }
}

impl DequeMessageQueue for ScriptedDequeMessageQueue {
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError> {
    let outcome = self
      .enqueue_first_outcomes
      .with_lock(|enqueue_first_outcomes| enqueue_first_outcomes.pop_front())
      .expect("enqueue_first outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.with_lock(|messages| messages.push_front(envelope));
        Ok(())
      },
      | ScriptedEnqueue::Full => {
        if let Some(hook) = self.enqueue_first_full_hook.with_lock(|full_hook| full_hook.take()) {
          hook.before_error_tx.send(()).expect("full hook notification must be delivered");
          hook.resume_rx.recv().expect("full hook resume signal must be delivered");
        }
        Err(SendError::full(envelope.into_payload()))
      },
      | ScriptedEnqueue::Closed => Err(SendError::closed(envelope.into_payload())),
    }
  }
}

enum BlockingInvocationKind {
  User,
  System,
}

struct BlockingInvoker {
  block_kind:         BlockingInvocationKind,
  entered_tx:         Sender<()>,
  resume_rx:          SharedLock<Receiver<()>>,
  user_invocations:   Arc<AtomicUsize>,
  system_invocations: Arc<AtomicUsize>,
}

impl BlockingInvoker {
  fn new(
    block_kind: BlockingInvocationKind,
    entered_tx: Sender<()>,
    resume_rx: Receiver<()>,
    user_invocations: Arc<AtomicUsize>,
    system_invocations: Arc<AtomicUsize>,
  ) -> Self {
    Self {
      block_kind,
      entered_tx,
      resume_rx: SharedLock::new_with_driver::<SpinSyncMutex<_>>(resume_rx),
      user_invocations,
      system_invocations,
    }
  }

  fn block_once(&self) {
    self.entered_tx.send(()).expect("blocking invoker should signal entry");
    self.resume_rx.with_lock(|resume_rx| resume_rx.recv().expect("blocking invoker should receive resume"));
  }
}

impl MessageInvoker for BlockingInvoker {
  fn invoke(&mut self, _message: AnyMessage) -> Result<(), ActorError> {
    let previous = self.user_invocations.fetch_add(1, Ordering::SeqCst);
    if matches!(self.block_kind, BlockingInvocationKind::User) && previous == 0 {
      self.block_once();
    }
    Ok(())
  }

  fn system_invoke(&mut self, _message: SystemMessage) -> Result<(), ActorError> {
    let previous = self.system_invocations.fetch_add(1, Ordering::SeqCst);
    if matches!(self.block_kind, BlockingInvocationKind::System) && previous == 0 {
      self.block_once();
    }
    Ok(())
  }
}

/// Test-only invoker that counts invocations without blocking.
///
/// Used by the AC-H1 tests that exercise `run()` single-threaded (no need to
/// coordinate with the runner). The counters are exposed via
/// `Arc<AtomicUsize>` so the test body can assert on them after `run()`
/// returns.
struct CountingInvoker {
  user_invocations:   Arc<AtomicUsize>,
  system_invocations: Arc<AtomicUsize>,
}

impl CountingInvoker {
  fn new(user_invocations: Arc<AtomicUsize>, system_invocations: Arc<AtomicUsize>) -> Self {
    Self { user_invocations, system_invocations }
  }
}

impl MessageInvoker for CountingInvoker {
  fn invoke(&mut self, _message: AnyMessage) -> Result<(), ActorError> {
    self.user_invocations.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  fn system_invoke(&mut self, _message: SystemMessage) -> Result<(), ActorError> {
    self.system_invocations.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }
}

struct CleanupCountingQueue {
  messages:       SharedLock<VecDeque<Envelope>>,
  clean_up_calls: Arc<AtomicUsize>,
}

impl CleanupCountingQueue {
  fn new(clean_up_calls: Arc<AtomicUsize>) -> Self {
    Self { messages: SharedLock::new_with_driver::<SpinSyncMutex<_>>(VecDeque::new()), clean_up_calls }
  }
}

impl MessageQueue for CleanupCountingQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.messages.with_lock(|messages| messages.push_back(envelope));
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.messages.with_lock(|messages| messages.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.messages.with_lock(|messages| messages.len())
  }

  fn clean_up(&self) {
    self.clean_up_calls.fetch_add(1, Ordering::SeqCst);
    self.messages.with_lock(|messages| messages.clear());
  }
}

fn expect_next_user_message(mailbox: &Mailbox, expected: &str) {
  let envelope = mailbox.dequeue().expect("user message expected");
  assert_eq!(envelope.payload().downcast_ref::<&str>().copied(), Some(expected));
}

#[test]
fn mailbox_new() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let _ = mailbox;
}

#[test]
fn mailbox_enqueue_user_returns_closed_when_queue_enqueue_returns_closed() {
  // Scripted queue enqueue surfaces `SendError::Closed` independently of
  // mailbox close state; the wrapper must forward it verbatim.
  let outcomes = VecDeque::from([ScriptedEnqueue::Closed]);
  let queue = Box::new(ScriptedMessageQueue::new(outcomes));
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);

  let result = mailbox.enqueue_user(AnyMessage::new("msg"));
  assert!(matches!(result, Err(SendError::Closed(_))));
}

#[test]
fn mailbox_set_instrumentation() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(1, 0);
  let instrumentation = MailboxInstrumentation::new(system_state, pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);
}

#[test]
fn mailbox_enqueue_system_after_system_state_drop_does_not_panic() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(2, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(8), Some(4), Some(6));
  mailbox.set_instrumentation(instrumentation);

  drop(system_state);

  let result = mailbox.enqueue_system(SystemMessage::Stop);
  assert!(result.is_ok());
}

#[test]
fn mailbox_enqueue_system() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = SystemMessage::Stop;
  let result = mailbox.enqueue_system(message);
  assert!(result.is_ok());
}

#[test]
fn mailbox_enqueue_user_unbounded() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_ok());
}

/// MB-H1: Pekko contract requires `enqueue` to always accept envelopes even
/// when the mailbox is suspended. Suspension only blocks dequeue; new
/// messages must be buffered for delivery after resume.
///
/// Reference: Apache Pekko `Mailbox.scala` (`messageQueue.enqueue` is called
/// unconditionally; the suspend check lives in `processMailbox` / `dequeue`).
#[test]
fn mailbox_enqueue_user_accepts_when_suspended() {
  // Given: a suspended mailbox.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.suspend();

  // When: a user message is enqueued while suspended.
  let message = AnyMessage::new(42_u32);
  let result = mailbox.enqueue_user(message);

  // Then: enqueue succeeds and the message is buffered.
  assert!(result.is_ok(), "suspended mailbox must accept enqueue, got {result:?}");
  assert_eq!(mailbox.user_len(), 1, "envelope must be buffered while suspended");

  // And: dequeue remains blocked while the mailbox stays suspended.
  assert!(mailbox.dequeue().is_none(), "dequeue must stay blocked while suspended");

  // When: the mailbox resumes.
  mailbox.resume();

  // Then: the buffered message becomes visible for dispatch.
  let _envelope = mailbox.dequeue().expect("buffered message must be dequeuable after resume");
}

/// MB-H1: `enqueue_envelope` must also accept envelopes while suspended and
/// deliver them after resume, mirroring the user-level alias contract.
#[test]
fn mailbox_enqueue_envelope_accepts_when_suspended_and_delivers_after_resume() {
  // Given: a suspended mailbox.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.suspend();

  // When: an envelope is enqueued directly via the dispatcher entry point.
  let envelope = Envelope::new(AnyMessage::new("suspended-msg"));
  let result = mailbox.enqueue_envelope(envelope);

  // Then: enqueue succeeds even though the mailbox is suspended.
  assert!(result.is_ok(), "suspended mailbox must accept enqueue_envelope, got {result:?}");
  assert_eq!(mailbox.user_len(), 1);
  assert!(mailbox.dequeue().is_none(), "dequeue must stay blocked while suspended");

  // When: the mailbox resumes.
  mailbox.resume();

  // Then: the buffered envelope is delivered in order.
  expect_next_user_message(&mailbox, "suspended-msg");
}

/// MB-H1 + close precedence: a closed mailbox that is also suspended must
/// return `Closed` (never `Suspended`) from `enqueue_user`, because the
/// suspend rejection is no longer a mailbox concern.
#[test]
fn mailbox_enqueue_user_returns_closed_when_closed_and_suspended() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.suspend();
  mailbox.become_closed();

  let result = mailbox.enqueue_user(AnyMessage::new("msg"));

  assert!(matches!(result, Err(SendError::Closed(_))), "closed wins over suspended, got {result:?}");
}

/// MB-H1: `prepend_user_messages_deque` must also accept while suspended and
/// deliver after resume, keeping parity with `enqueue_envelope`.
#[test]
fn mailbox_prepend_user_messages_deque_accepts_when_suspended_and_delivers_after_resume() {
  // Given: a suspended deque-capable mailbox with one existing message.
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);
  mailbox.enqueue_user(AnyMessage::new("existing")).expect("existing");
  mailbox.suspend();

  // When: two messages are prepended while suspended.
  let prepended = VecDeque::from([AnyMessage::new("first"), AnyMessage::new("second")]);
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  let result = mailbox.prepend_user_messages_deque(user_deque, &prepended);

  // Then: prepend succeeds and the buffered messages remain undelivered.
  assert!(result.is_ok(), "suspended mailbox must accept prepend, got {result:?}");
  assert_eq!(mailbox.user_len(), 3);
  assert!(mailbox.dequeue().is_none(), "dequeue must stay blocked while suspended");

  // When: the mailbox resumes.
  mailbox.resume();

  // Then: prepended messages are delivered before the pre-existing one.
  expect_next_user_message(&mailbox, "first");
  expect_next_user_message(&mailbox, "second");
  expect_next_user_message(&mailbox, "existing");
}

/// MB-H1 + close precedence: a closed + suspended mailbox must reject
/// `prepend_user_messages_deque` with `Closed` (never `Suspended`).
#[test]
fn mailbox_prepend_user_messages_deque_returns_closed_when_closed_and_suspended() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);
  mailbox.suspend();
  mailbox.become_closed();

  let messages = VecDeque::from([AnyMessage::new("msg")]);
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  let result = mailbox.prepend_user_messages_deque(user_deque, &messages);

  assert!(matches!(result, Err(SendError::Closed(_))), "closed wins over suspended, got {result:?}");
}

/// MB-H2: Pekko's `Mailbox.cleanUp` drains **both** the user and the system
/// queues into the deadLetterMailbox. Before this fix, `finalize_cleanup`
/// drained only the user queue, so `Terminated` / `Watch` / `Create` /
/// `Stop` accumulated during shutdown were silently discarded. The current
/// contract routes them through the dead-letter sink so operators can
/// observe lost system messages, and this test pins that behaviour.
///
/// Reference: Apache Pekko `Mailbox.scala#cleanUp` (L288-352).
#[test]
fn finalize_cleanup_drains_system_queue_to_dead_letters() {
  // Given: a mailbox with instrumentation installed so the system_state
  // dead-letter sink is reachable, and two system messages queued.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(7, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);

  mailbox.enqueue_system(SystemMessage::Stop).expect("stop must enqueue");
  mailbox.enqueue_system(SystemMessage::Create).expect("create must enqueue");

  // When: cleanup runs (DrainToDeadLetters policy is the default).
  mailbox.become_closed();

  // Then: every enqueued system message must appear in DL storage in FIFO
  // order, tagged with the mailbox's pid and `Dropped` reason (reusing the
  // same reason as the user-queue drain for symmetry).
  let entries = system_state.dead_letters();
  assert_eq!(entries.len(), 2, "system queue drain must route every message to DL: {entries:?}");

  let first_msg = entries[0].message().downcast_ref::<SystemMessage>().expect("DL payload must wrap SystemMessage");
  assert_eq!(*first_msg, SystemMessage::Stop, "FIFO order must be preserved");
  assert_eq!(entries[0].reason(), DeadLetterReason::Dropped);
  assert_eq!(entries[0].recipient(), Some(pid));

  let second_msg = entries[1].message().downcast_ref::<SystemMessage>().expect("DL payload must wrap SystemMessage");
  assert_eq!(*second_msg, SystemMessage::Create, "FIFO order must be preserved");
  assert_eq!(entries[1].reason(), DeadLetterReason::Dropped);
  assert_eq!(entries[1].recipient(), Some(pid));
}

/// MB-H2: When both user and system queues contain messages, cleanup must
/// drain both into DL. Order between user and system is an implementation
/// detail (Pekko drains system first, then user), but every message must
/// surface exactly once.
#[test]
fn finalize_cleanup_drains_both_user_and_system_queues_to_dead_letters() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(8, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);

  mailbox.enqueue_user(AnyMessage::new("u1")).expect("u1");
  mailbox.enqueue_user(AnyMessage::new("u2")).expect("u2");
  mailbox.enqueue_system(SystemMessage::Stop).expect("stop");

  mailbox.become_closed();

  let entries = system_state.dead_letters();
  assert_eq!(entries.len(), 3, "both queues must be drained: {entries:?}");

  // Every entry must be attributed to the mailbox owner and classified as Dropped.
  for entry in &entries {
    assert_eq!(entry.reason(), DeadLetterReason::Dropped);
    assert_eq!(entry.recipient(), Some(pid));
  }

  // The system message must appear at least once across the drain.
  let system_hits = entries
    .iter()
    .filter(|entry| entry.message().downcast_ref::<SystemMessage>() == Some(&SystemMessage::Stop))
    .count();
  assert_eq!(system_hits, 1, "Stop must be routed to DL exactly once");

  // Both user payloads must appear exactly once, preserving their labels.
  let user_labels: Vec<&str> =
    entries.iter().filter_map(|entry| entry.message().downcast_ref::<&str>().copied()).collect();
  assert_eq!(user_labels.len(), 2, "both user envelopes must surface in DL");
  assert!(user_labels.contains(&"u1"), "u1 must surface: {user_labels:?}");
  assert!(user_labels.contains(&"u2"), "u2 must surface: {user_labels:?}");
}

/// MB-H2: Sharing mailboxes use [`MailboxCleanupPolicy::LeaveSharedQueue`].
/// The `LeaveSharedQueue` policy applies **only** to the user queue
/// (which is shared across multiple actor cells and must not be drained
/// on a single cell's shutdown). The system queue, however, is owned
/// exclusively by each mailbox and must always be drained to dead letters
/// to preserve observability — Pekko's `Mailbox.cleanUp()` drains the
/// system queue unconditionally regardless of user-queue sharing policy.
/// See Bugbot feedback on PR #1594 for the contract clarification.
#[test]
fn finalize_cleanup_leave_shared_queue_still_drains_system_queue() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_sharing(MailboxPolicy::unbounded(None), queue);
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(9, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);

  mailbox.enqueue_system(SystemMessage::Stop).expect("stop");

  mailbox.become_closed();

  let entries = system_state.dead_letters();
  assert_eq!(entries.len(), 1, "LeaveSharedQueue cleanup must still drain the system queue, got {entries:?}");
  assert_eq!(
    entries[0].message().downcast_ref::<SystemMessage>(),
    Some(&SystemMessage::Stop),
    "the Stop system message must be routed to DL",
  );
}

/// MB-H2 safety: without instrumentation (no `system_state` weak ref),
/// `finalize_cleanup` must remain panic-free even when the system queue is
/// non-empty. The system messages are dropped locally (no sink is available
/// to observe them), and the mailbox still transitions to cleaned state.
#[test]
fn finalize_cleanup_without_system_state_does_not_panic() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  // Intentionally no set_instrumentation — system_state() returns None.
  mailbox.enqueue_system(SystemMessage::Stop).expect("stop");
  mailbox.enqueue_system(SystemMessage::Create).expect("create");

  mailbox.become_closed();

  assert!(mailbox.is_closed(), "cleanup must complete the close transition");
}

#[test]
fn mailbox_enqueue_user_bounded() {
  use core::num::NonZeroUsize;

  let capacity = NonZeroUsize::new(10).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let mailbox = Mailbox::new(policy);
  let message = AnyMessage::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_ok());
}

/// MB-H3: Pekko's `BoundedNodeMessageQueue.enqueue` routes a DropNewest
/// rejection through `deadLetters.tell(DeadLetter(...))` and returns void.
/// The rejected envelope must surface on the dead-letter sink with reason
/// `MailboxFull` so operators observe the loss, and the caller observes
/// `Ok(())` because the mailbox is the sole DL recorder for overflow
/// (Pekko parity — no double recording at upstream layers).
///
/// Reference: Apache Pekko `Mailbox.scala` L426-432 (BoundedNodeMessageQueue).
#[test]
fn mailbox_enqueue_drop_newest_records_dead_letter_on_overflow() {
  use core::num::NonZeroUsize;

  // Given: a DropNewest-bounded mailbox (capacity 1) + instrumentation so
  // the dead-letter sink is reachable.
  let capacity = NonZeroUsize::new(1).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let mailbox = Mailbox::new(policy);
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(10, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(1), None, None);
  mailbox.set_instrumentation(instrumentation);

  // Fill the mailbox to capacity.
  mailbox.enqueue_user(AnyMessage::new("first")).expect("first must enqueue");

  // When: a second message overflows the bounded queue.
  let result = mailbox.enqueue_user(AnyMessage::new("rejected"));

  // Then: the caller observes Ok(()) (Pekko void-on-success contract).
  assert!(result.is_ok(), "DropNewest overflow must report success (mailbox handles DL internally), got {result:?}");

  // And: the rejected envelope must appear in the DL sink with
  // `MailboxFull` so the loss is observable.
  let entries = system_state.dead_letters();
  assert_eq!(entries.len(), 1, "DropNewest overflow must record exactly one DL entry, got {entries:?}");
  assert_eq!(entries[0].reason(), DeadLetterReason::MailboxFull);
  assert_eq!(entries[0].recipient(), Some(pid));
  assert_eq!(
    entries[0].message().downcast_ref::<&str>().copied(),
    Some("rejected"),
    "DL entry must carry the rejected payload",
  );

  // And: the mailbox state is unchanged — the first message is still queued.
  assert_eq!(mailbox.user_len(), 1, "existing message must stay queued");
}

/// MB-H3: Pekko's BoundedNodeMessageQueue + DropOldest evicts an existing
/// envelope and accepts the incoming one. The evicted envelope must be
/// routed to DeadLetters with reason `MailboxFull` (not silently dropped),
/// while the caller observes `Ok(())` because the enqueue itself succeeded.
#[test]
fn mailbox_enqueue_drop_oldest_records_dead_letter_for_evicted_envelope() {
  use core::num::NonZeroUsize;

  // Given: a DropOldest-bounded mailbox (capacity 1) + instrumentation.
  let capacity = NonZeroUsize::new(1).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropOldest, None);
  let mailbox = Mailbox::new(policy);
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(11, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(1), None, None);
  mailbox.set_instrumentation(instrumentation);

  // Fill the mailbox to capacity — "evicted" is the envelope pushed out.
  mailbox.enqueue_user(AnyMessage::new("evicted")).expect("first must enqueue");

  // When: a second message triggers DropOldest eviction.
  let result = mailbox.enqueue_user(AnyMessage::new("accepted"));

  // Then: the caller observes success — the incoming message is accepted.
  assert!(result.is_ok(), "DropOldest eviction must still accept the new message, got {result:?}");

  // And: the evicted envelope surfaces in DL with `MailboxFull`.
  let entries = system_state.dead_letters();
  assert_eq!(entries.len(), 1, "DropOldest eviction must record exactly one DL entry, got {entries:?}");
  assert_eq!(entries[0].reason(), DeadLetterReason::MailboxFull);
  assert_eq!(entries[0].recipient(), Some(pid));
  assert_eq!(
    entries[0].message().downcast_ref::<&str>().copied(),
    Some("evicted"),
    "DL entry must carry the evicted payload (not the incoming one)",
  );

  // And: the accepted message is queued and is the one that dequeues.
  assert_eq!(mailbox.user_len(), 1);
  let envelope = mailbox.dequeue().expect("accepted envelope must be dequeuable");
  assert_eq!(envelope.payload().downcast_ref::<&str>().copied(), Some("accepted"));
}

/// MB-H3: The Grow strategy never evicts — every enqueue past nominal
/// capacity is still accepted, so no DeadLetter entries should ever appear
/// from the mailbox layer on this path.
#[test]
fn mailbox_enqueue_grow_does_not_record_dead_letter() {
  use core::num::NonZeroUsize;

  let capacity = NonZeroUsize::new(1).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::Grow, None);
  let mailbox = Mailbox::new(policy);
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(12, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(1), None, None);
  mailbox.set_instrumentation(instrumentation);

  // Enqueue past nominal capacity — Grow must keep accepting.
  mailbox.enqueue_user(AnyMessage::new("first")).expect("first");
  mailbox.enqueue_user(AnyMessage::new("second")).expect("second past capacity");
  mailbox.enqueue_user(AnyMessage::new("third")).expect("third past capacity");

  assert_eq!(mailbox.user_len(), 3);
  let entries = system_state.dead_letters();
  assert!(entries.is_empty(), "Grow strategy must never record DL entries, got {entries:?}");
}

#[test]
fn mailbox_prepend_user_messages_deque_returns_error_and_keeps_existing_messages() {
  let queue = Box::new(ScriptedDequeMessageQueue::new(
    VecDeque::from([
      ScriptedEnqueue::Enqueued, // existing-1
      ScriptedEnqueue::Enqueued, // existing-2
    ]),
    VecDeque::from([ScriptedEnqueue::Full]),
  ));
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);

  mailbox.enqueue_user(AnyMessage::new("existing-1")).expect("existing-1");
  mailbox.enqueue_user(AnyMessage::new("existing-2")).expect("existing-2");

  let prepended = VecDeque::from([AnyMessage::new("prepend")]);
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  let result = mailbox.prepend_user_messages_deque(user_deque, &prepended);

  assert!(result.is_err(), "prepend should fail: {result:?}");
  assert_eq!(mailbox.user_len(), 2, "existing messages must remain queued");
  expect_next_user_message(&mailbox, "existing-1");
  expect_next_user_message(&mailbox, "existing-2");
}

#[test]
fn mailbox_prepend_user_messages_deque_blocks_concurrent_enqueue_until_prepend_finishes() {
  use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  let (before_error_tx, before_error_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let queue = Box::new(ScriptedDequeMessageQueue::new_with_enqueue_first_hook(
    VecDeque::from([
      ScriptedEnqueue::Enqueued, // existing-1
      ScriptedEnqueue::Enqueued, // existing-2
      ScriptedEnqueue::Enqueued, // concurrent enqueue
    ]),
    VecDeque::from([ScriptedEnqueue::Full]),
    Some(ScriptedFullHook::new(before_error_tx, resume_rx)),
  ));
  let mailbox = Arc::new(Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue));

  mailbox.enqueue_user(AnyMessage::new("existing-1")).expect("existing-1");
  mailbox.enqueue_user(AnyMessage::new("existing-2")).expect("existing-2");

  let mailbox_for_prepend = Arc::clone(&mailbox);
  let prepended = VecDeque::from([AnyMessage::new("prepend")]);
  let prepend_handle = thread::spawn(move || {
    let user_deque = mailbox_for_prepend.user_deque().expect("deque mailbox should expose deque capability");
    mailbox_for_prepend.prepend_user_messages_deque(user_deque, &prepended)
  });

  before_error_rx.recv().expect("prepend がスクリプト上の full 地点に到達するべき");

  let mailbox_for_enqueue = Arc::clone(&mailbox);
  let (enqueue_done_tx, enqueue_done_rx) = mpsc::channel();
  let enqueue_handle = thread::spawn(move || {
    let result = mailbox_for_enqueue.enqueue_user(AnyMessage::new("concurrent"));
    enqueue_done_tx.send(()).expect("エンキュー完了シグナルが送信されるべき");
    result
  });

  assert!(
    enqueue_done_rx.recv_timeout(Duration::from_millis(200)).is_err(),
    "prepend 中は並行エンキューがブロックされるべき",
  );

  resume_tx.send(()).expect("prepend 再開シグナルが送信されるべき");

  let prepend_result = prepend_handle.join().expect("prepend スレッドが完了するべき");
  assert!(matches!(prepend_result, Err(SendError::Full(_))));

  let enqueue_result = enqueue_handle.join().expect("エンキュースレッドが完了するべき");
  assert!(enqueue_result.is_ok());
  assert_eq!(mailbox.user_len(), 3);
}

#[test]
fn mailbox_prepend_user_messages_deque_is_noop_for_empty_batch() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);

  mailbox.enqueue_user(AnyMessage::new("existing")).expect("existing");

  let empty = VecDeque::new();
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  let result = mailbox.prepend_user_messages_deque(user_deque, &empty);

  assert!(result.is_ok());
  assert_eq!(mailbox.user_len(), 1);
  expect_next_user_message(&mailbox, "existing");
}

#[test]
fn mailbox_prepend_user_messages_deque_preserves_front_insertion_order() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);

  mailbox.enqueue_user(AnyMessage::new("existing")).expect("existing");

  let prepended = VecDeque::from([AnyMessage::new("first"), AnyMessage::new("second")]);
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  mailbox.prepend_user_messages_deque(user_deque, &prepended).expect("deque prepend should succeed");

  assert_eq!(mailbox.user_len(), 3);
  expect_next_user_message(&mailbox, "first");
  expect_next_user_message(&mailbox, "second");
  expect_next_user_message(&mailbox, "existing");
}

#[test]
fn mailbox_dequeue_empty() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  assert!(mailbox.dequeue().is_none(), "empty mailbox must yield no user message");
  assert_eq!(mailbox.system_len(), 0, "empty mailbox must have no system messages");
}

#[test]
fn mailbox_dequeue_user_message() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  assert!(mailbox.dequeue().is_some(), "enqueued user message must be dequeuable");
}

/// Pins Pekko's `processMailbox` / `processAllSystemMessages` contract through
/// `run()`: when both system and user messages are enqueued, the system drain
/// must fire **before** any user message, as observed via invoker event order.
#[test]
fn mailbox_run_drains_system_before_user() {
  use core::num::NonZeroUsize;

  struct OrderRecordingInvoker {
    log: Arc<SpinSyncMutex<Vec<&'static str>>>,
  }

  impl MessageInvoker for OrderRecordingInvoker {
    fn invoke(&mut self, _message: AnyMessage) -> Result<(), ActorError> {
      self.log.lock().push("user");
      Ok(())
    }

    fn system_invoke(&mut self, _message: SystemMessage) -> Result<(), ActorError> {
      self.log.lock().push("system");
      Ok(())
    }
  }

  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.enqueue_user(AnyMessage::new(1_u32)).unwrap();
  mailbox.enqueue_system(SystemMessage::Stop).unwrap();

  let order = Arc::new(SpinSyncMutex::new(Vec::<&'static str>::new()));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(OrderRecordingInvoker { log: Arc::clone(&order) })));

  let throughput = NonZeroUsize::new(10).unwrap();
  let needs_reschedule = mailbox.run(throughput, None);

  let observed = order.lock().clone();
  assert_eq!(observed.first(), Some(&"system"), "system drain must precede user processing: {observed:?}");
  assert_eq!(observed.iter().filter(|k| **k == "system").count(), 1);
  assert_eq!(observed.iter().filter(|k| **k == "user").count(), 1);
  assert!(!needs_reschedule, "single system + user drain should leave no pending work");
}

#[test]
fn mailbox_dequeue_suspended() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  mailbox.suspend();
  assert!(mailbox.dequeue().is_none(), "suspended mailbox must not yield user messages");
}

#[test]
fn mailbox_suspend_resume() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  assert!(!mailbox.is_suspended());
  mailbox.suspend();
  assert!(mailbox.is_suspended());
  mailbox.resume();
  assert!(!mailbox.is_suspended());
}

#[test]
fn mailbox_user_len() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  assert_eq!(mailbox.user_len(), 0);
  mailbox.enqueue_user(AnyMessage::new(1_u32)).unwrap();
  assert_eq!(mailbox.user_len(), 1);
  mailbox.enqueue_user(AnyMessage::new(2_u32)).unwrap();
  assert_eq!(mailbox.user_len(), 2);
  assert!(mailbox.dequeue().is_some());
  assert_eq!(mailbox.user_len(), 1);
}

#[test]
fn mailbox_system_len() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  assert_eq!(mailbox.system_len(), 0);
  mailbox.enqueue_system(SystemMessage::Stop).unwrap();
  assert_eq!(mailbox.system_len(), 1);
  mailbox.enqueue_system(SystemMessage::Stop).unwrap();
  assert_eq!(mailbox.system_len(), 2);
  assert!(mailbox.dequeue_system().is_some());
  assert_eq!(mailbox.system_len(), 1);
}

#[test]
fn mailbox_throughput_limit() {
  use core::num::NonZeroUsize;

  let limit = NonZeroUsize::new(100).unwrap();
  let policy = MailboxPolicy::unbounded(Some(limit));
  let mailbox = Mailbox::new(policy);
  assert_eq!(mailbox.throughput_limit(), Some(limit));

  let policy_no_limit = MailboxPolicy::unbounded(None);
  let mailbox_no_limit = Mailbox::new(policy_no_limit);
  assert_eq!(mailbox_no_limit.throughput_limit(), None);
}

#[test]
fn mailbox_is_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  assert!(!mailbox.is_closed());
  mailbox.become_closed();
  assert!(mailbox.is_closed());
}

#[test]
fn mailbox_enqueue_envelope_returns_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.become_closed();

  let result = mailbox.enqueue_envelope(Envelope::new(AnyMessage::new("msg")));
  assert!(matches!(result, Err(SendError::Closed(_))), "expected Closed, got {result:?}");
}

#[test]
fn mailbox_enqueue_user_returns_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.become_closed();

  let result = mailbox.enqueue_user(AnyMessage::new("msg"));
  assert!(matches!(result, Err(SendError::Closed(_))), "expected Closed, got {result:?}");
}

#[test]
fn mailbox_prepend_user_messages_deque_returns_closed_after_mailbox_close() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);
  mailbox.become_closed();

  let messages = VecDeque::from([AnyMessage::new("msg")]);
  let user_deque = mailbox.user_deque().expect("deque mailbox should expose deque capability");
  let result = mailbox.prepend_user_messages_deque(user_deque, &messages);
  assert!(matches!(result, Err(SendError::Closed(_))), "expected Closed, got {result:?}");
}

/// Regression for the "cleanup wins the lock race" scenario: a producer
/// that has already passed the fast path and is executing the locked
/// critical section must observe the authoritative `is_closed()` re-check
/// when cleanup has closed the state.
///
/// The test thread takes `put_lock`, starts a producer, waits until the
/// producer is blocked on the same lock, then closes and cleans the mailbox
/// while still holding the lock. Once the lock is released, the producer must
/// observe the under-lock `is_closed()` re-check and fail with `Closed`.
#[test]
fn cleanup_close_wins_against_inflight_enqueue() {
  use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  let mailbox = Arc::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let (lock_held_tx, lock_held_rx) = mpsc::channel();
  let (release_lock_tx, release_lock_rx) = mpsc::channel();
  let mailbox_for_lock = Arc::clone(&mailbox);
  let lock_handle = thread::spawn(move || {
    mailbox_for_lock.put_lock.with_lock(|_| {
      lock_held_tx.send(()).expect("lock 取得シグナルが送信されるべき");
      release_lock_rx.recv().expect("lock 解放シグナルを受信できるべき");
    });
  });
  lock_held_rx.recv().expect("lock 保持スレッドが起動するべき");
  let (started_tx, started_rx) = mpsc::channel();
  let (result_tx, result_rx) = mpsc::channel();
  let mailbox_for_enqueue = Arc::clone(&mailbox);
  let enqueue_handle = thread::spawn(move || {
    started_tx.send(()).expect("enqueue 開始シグナルが送信されるべき");
    let result = mailbox_for_enqueue.enqueue_user(AnyMessage::new("inflight"));
    result_tx.send(result).expect("enqueue 結果が送信されるべき");
  });

  started_rx.recv().expect("enqueue スレッドが起動するべき");
  assert!(result_rx.recv_timeout(Duration::from_millis(200)).is_err(), "producer は put_lock 上でブロックされるべき",);

  assert_eq!(mailbox.state.request_close(), CloseRequestOutcome::CallerOwnsFinalizer);
  mailbox.user.clean_up();
  mailbox.state.finish_cleanup();
  release_lock_tx.send(()).expect("lock 解放シグナルが送信されるべき");

  let result = result_rx.recv().expect("enqueue 結果を受信できるべき");
  enqueue_handle.join().expect("enqueue スレッドが完了するべき");
  lock_handle.join().expect("lock 保持スレッドが完了するべき");
  assert!(
    matches!(result, Err(SendError::Closed(_))),
    "under-lock re-check must reject inflight enqueue, got {result:?}",
  );
  assert_eq!(mailbox.user_len(), 0, "no phantom message should be in the queue");
}

/// Same invariant as [`cleanup_close_wins_against_inflight_enqueue`] but
/// exercising the deque-only prepend path used by
/// `ActorCell::unstash_*`.
#[test]
fn cleanup_close_wins_against_inflight_prepend() {
  use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Arc::new(Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue));
  let (lock_held_tx, lock_held_rx) = mpsc::channel();
  let (release_lock_tx, release_lock_rx) = mpsc::channel();
  let mailbox_for_lock = Arc::clone(&mailbox);
  let lock_handle = thread::spawn(move || {
    mailbox_for_lock.put_lock.with_lock(|_| {
      lock_held_tx.send(()).expect("lock 取得シグナルが送信されるべき");
      release_lock_rx.recv().expect("lock 解放シグナルを受信できるべき");
    });
  });
  lock_held_rx.recv().expect("lock 保持スレッドが起動するべき");
  let messages = VecDeque::from([AnyMessage::new("inflight-prepend")]);
  let (started_tx, started_rx) = mpsc::channel();
  let (result_tx, result_rx) = mpsc::channel();
  let mailbox_for_prepend = Arc::clone(&mailbox);
  let prepend_handle = thread::spawn(move || {
    started_tx.send(()).expect("prepend 開始シグナルが送信されるべき");
    let user_deque = mailbox_for_prepend.user_deque().expect("deque mailbox should expose deque capability");
    let result = mailbox_for_prepend.prepend_user_messages_deque(user_deque, &messages);
    result_tx.send(result).expect("prepend 結果が送信されるべき");
  });

  started_rx.recv().expect("prepend スレッドが起動するべき");
  assert!(result_rx.recv_timeout(Duration::from_millis(200)).is_err(), "prepend は put_lock 上でブロックされるべき",);

  assert_eq!(mailbox.state.request_close(), CloseRequestOutcome::CallerOwnsFinalizer);
  mailbox.user.clean_up();
  mailbox.state.finish_cleanup();
  release_lock_tx.send(()).expect("lock 解放シグナルが送信されるべき");

  let result = result_rx.recv().expect("prepend 結果を受信できるべき");
  prepend_handle.join().expect("prepend スレッドが完了するべき");
  lock_handle.join().expect("lock 保持スレッドが完了するべき");
  assert!(
    matches!(result, Err(SendError::Closed(_))),
    "under-lock re-check must reject inflight prepend, got {result:?}",
  );
  assert_eq!(mailbox.user_len(), 0, "no phantom prepended message should be in the queue");
}

#[test]
fn runner_finalizer_cleans_up_exactly_once() {
  use core::num::NonZeroUsize;
  use std::{sync::mpsc, thread};

  let clean_up_calls = Arc::new(AtomicUsize::new(0));
  let queue = Box::new(CleanupCountingQueue::new(clean_up_calls.clone()));
  let mailbox = Arc::new(Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue));
  let (entered_tx, entered_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(BlockingInvoker::new(
    BlockingInvocationKind::User,
    entered_tx,
    resume_rx,
    user_invocations.clone(),
    system_invocations,
  ))));
  mailbox.enqueue_user(AnyMessage::new("first")).expect("first");
  mailbox.enqueue_user(AnyMessage::new("second")).expect("second");

  let mailbox_for_run = Arc::clone(&mailbox);
  let run_handle = thread::spawn(move || mailbox_for_run.run(NonZeroUsize::new(8).unwrap(), None));

  entered_rx.recv().expect("runner should start first user message");
  mailbox.become_closed();
  mailbox.become_closed();
  resume_tx.send(()).expect("resume");

  assert!(!run_handle.join().expect("run thread should complete"));
  assert_eq!(user_invocations.load(Ordering::SeqCst), 1, "second queued message must not be delivered");
  assert_eq!(mailbox.user_len(), 0, "runner finalizer should clean remaining user queue");
  assert_eq!(clean_up_calls.load(Ordering::SeqCst), 1, "cleanup must run exactly once");
}

#[test]
fn close_request_does_not_dequeue_additional_system_messages() {
  use core::num::NonZeroUsize;
  use std::{sync::mpsc, thread};

  let mailbox = Arc::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let (entered_tx, entered_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(BlockingInvoker::new(
    BlockingInvocationKind::System,
    entered_tx,
    resume_rx,
    user_invocations,
    system_invocations.clone(),
  ))));
  mailbox.enqueue_system(SystemMessage::Create).expect("create");
  mailbox.enqueue_system(SystemMessage::Stop).expect("stop");

  let mailbox_for_run = Arc::clone(&mailbox);
  let run_handle = thread::spawn(move || mailbox_for_run.run(NonZeroUsize::new(8).unwrap(), None));

  entered_rx.recv().expect("system invoker should block on first system message");
  mailbox.become_closed();
  resume_tx.send(()).expect("resume");

  assert!(!run_handle.join().expect("run thread should complete"));
  assert_eq!(system_invocations.load(Ordering::SeqCst), 1, "close request must stop the next system dequeue");
  // MB-H2: Pekko parity drains the system queue into the dead-letter sink
  // during `finalize_cleanup` (instead of leaving it queued). The second
  // `Stop` message is therefore removed from the queue — the runner just
  // never invoked it — so the post-cleanup queue length is 0.
  assert_eq!(mailbox.system_len(), 0, "cleanup must drain the remaining system message");
}

// =====================================================================
// AC-H1: Pekko `Mailbox.run()` parity tests
// ---------------------------------------------------------------------
// Pekko の `Mailbox.run()` (Mailbox.scala:228-278) は user / system 処理を
// 以下の 2 段階に分離している:
//
//   1. `processAllSystemMessages()` を **起動時** と **毎 user message 処理後** に呼び出す。
//      呼ばれるたびに system queue 全体を drain する (throughput に縛られない)。
//   2. `processMailbox()` は **user message を 1 件ずつ** dequeue し、**user 専用の throughput
//      カウンタ** を消費する。各 user message 間で `processAllSystemMessages()` を再実行し、
//      途中到着した Suspend/Resume/Stop 等が次の user 処理の前に反映されるようにする。
//
// AC-H1 テスト (T1-T5) はこの契約を pin する。本 PR 適用後は T1-T5 全てが green で、
// system message が user throughput budget を消費する旧実装 (単一カウンタ) への
// 回帰を防止する役割を担う。
// =====================================================================

/// AC-H1-T1: throughput is a user-only counter.
///
/// Given a mailbox with 5 user messages queued and throughput=2, a
/// single `run()` call must invoke exactly 2 users and leave 3 users in
/// the queue. The remainder is the dispatcher's cue to reschedule (the
/// `run()` return value is `true` because `user_len > 0`).
#[test]
fn ac_h1_t1_throughput_is_user_only_counter() {
  use core::num::NonZeroUsize;

  // Given: 5 user messages queued, CountingInvoker installed.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  for i in 0..5 {
    mailbox.enqueue_user(AnyMessage::new(i as u64)).expect("user enqueue should succeed");
  }

  // When: run with throughput = 2.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(2).unwrap(), None);

  // Then: exactly 2 user messages processed, 3 remain queued.
  assert_eq!(user_invocations.load(Ordering::SeqCst), 2, "throughput=2 must consume exactly 2 user messages");
  assert_eq!(system_invocations.load(Ordering::SeqCst), 0, "no system messages were queued");
  assert_eq!(mailbox.user_len(), 3, "3 user messages must remain for the next drain");
  assert!(needs_reschedule, "run() must signal pending work while user_len > 0");
}

/// AC-H1-T2: Suspend arriving during user 1 halts user 2 on the same run.
///
/// Given user messages [u1, u2] and a BlockingInvoker that blocks during
/// u1, when the test thread enqueues `SystemMessage::Suspend` before
/// releasing the invoker, the post-user system flush must apply Suspend
/// and the `should_process_message` guard must block u2 from being
/// dequeued within the same `run()` call.
#[test]
fn ac_h1_t2_suspend_midflight_blocks_next_user() {
  use core::num::NonZeroUsize;
  use std::{sync::mpsc, thread};

  // Given: BlockingInvoker that blocks on user 1; user 1 and user 2 queued.
  let mailbox = Arc::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let (entered_tx, entered_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(BlockingInvoker::new(
    BlockingInvocationKind::User,
    entered_tx,
    resume_rx,
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  mailbox.enqueue_user(AnyMessage::new("u1")).expect("u1 enqueue");
  mailbox.enqueue_user(AnyMessage::new("u2")).expect("u2 enqueue");

  // When: run starts, u1 blocks, test enqueues Suspend, releases invoker.
  let mailbox_for_run = Arc::clone(&mailbox);
  let run_handle = thread::spawn(move || mailbox_for_run.run(NonZeroUsize::new(10).unwrap(), None));
  entered_rx.recv().expect("runner should block inside u1 invoke");
  mailbox.enqueue_system(SystemMessage::Suspend).expect("suspend enqueue");
  resume_tx.send(()).expect("release u1");

  // Then: u1 processed, Suspend applied, u2 held back to the next run.
  let needs_reschedule = run_handle.join().expect("run thread must join");
  assert_eq!(user_invocations.load(Ordering::SeqCst), 1, "only u1 must be invoked; u2 is gated by Suspend");
  assert_eq!(mailbox.user_len(), 1, "u2 must remain queued for the next drain");
  assert!(mailbox.is_suspended(), "Suspend must have been applied by the post-user system flush");
  assert!(needs_reschedule, "run() must signal pending work while user_len > 0");
}

/// AC-H1-T3: all system messages are processed before any user message on entry.
///
/// Given 3 system messages queued ahead of 2 user messages with a
/// generous throughput budget, the single-threaded `run()` must invoke
/// every queued system message (via the invoker), then invoke both
/// users, leaving both queues empty. This pins Pekko's
/// `processAllSystemMessages()` entry-point contract.
#[test]
fn ac_h1_t3_system_messages_drained_before_users_on_entry() {
  use core::num::NonZeroUsize;

  // Given: 3 system messages + 2 user messages queued, CountingInvoker installed.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  mailbox.enqueue_system(SystemMessage::Create).expect("create enqueue");
  mailbox.enqueue_system(SystemMessage::Watch(Pid::new(42, 0))).expect("watch enqueue");
  mailbox.enqueue_system(SystemMessage::Stop).expect("stop enqueue");
  mailbox.enqueue_user(AnyMessage::new("u1")).expect("u1 enqueue");
  mailbox.enqueue_user(AnyMessage::new("u2")).expect("u2 enqueue");

  // When: run with ample throughput.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(10).unwrap(), None);

  // Then: every queued message invoked; both queues empty.
  assert_eq!(system_invocations.load(Ordering::SeqCst), 3, "all 3 system messages must reach the invoker");
  assert_eq!(user_invocations.load(Ordering::SeqCst), 2, "both user messages must be invoked after system flush");
  assert_eq!(mailbox.system_len(), 0, "system queue must be empty post-drain");
  assert_eq!(mailbox.user_len(), 0, "user queue must be empty post-drain");
  assert!(!needs_reschedule, "run() must return false when queues are drained and no reschedule is pending");
}

/// AC-H1-T4: system messages must not consume the user throughput budget.
///
/// Given 5 system messages queued ahead of 2 user messages with
/// throughput=2, the new implementation must drain all 5 system
/// messages (they are unmetered) and then consume its 2-message
/// throughput budget on the 2 user messages. The current implementation
/// fails this test because its shared counter consumes the budget on
/// the first 2 system messages, starving user delivery.
#[test]
fn ac_h1_t4_system_messages_do_not_consume_user_throughput() {
  use core::num::NonZeroUsize;

  // Given: 5 system messages + 2 user messages queued, throughput = 2.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  for _ in 0..5 {
    mailbox.enqueue_system(SystemMessage::Create).expect("system enqueue");
  }
  mailbox.enqueue_user(AnyMessage::new("u1")).expect("u1 enqueue");
  mailbox.enqueue_user(AnyMessage::new("u2")).expect("u2 enqueue");

  // When: run with throughput = 2.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(2).unwrap(), None);

  // Then: all 5 system messages drained (unmetered); both user messages
  // processed (throughput=2 spent on users only).
  assert_eq!(
    system_invocations.load(Ordering::SeqCst),
    5,
    "all system messages must drain regardless of user throughput",
  );
  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    2,
    "user throughput=2 must be spent entirely on user messages; system drain is free",
  );
  assert_eq!(mailbox.system_len(), 0, "system queue must be fully drained");
  assert_eq!(mailbox.user_len(), 0, "user queue must be fully drained");
  assert!(!needs_reschedule, "run() must return false when both queues are drained");
}

/// AC-H1-T5: Resume arriving mid-run re-enables user processing.
///
/// Given a mailbox that blocks on user 1 (via BlockingInvoker), when
/// the test thread enqueues Suspend **followed by** Resume before
/// releasing u1, the post-user system flush must apply Suspend then
/// Resume — leaving the mailbox un-suspended — so user 2 can be
/// dequeued in the same `run()` call.
#[test]
fn ac_h1_t5_resume_in_system_flush_reenables_next_user() {
  use core::num::NonZeroUsize;
  use std::{sync::mpsc, thread};

  // Given: BlockingInvoker(User) + 2 users queued.
  let mailbox = Arc::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let (entered_tx, entered_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(BlockingInvoker::new(
    BlockingInvocationKind::User,
    entered_tx,
    resume_rx,
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  mailbox.enqueue_user(AnyMessage::new("u1")).expect("u1 enqueue");
  mailbox.enqueue_user(AnyMessage::new("u2")).expect("u2 enqueue");

  // When: run starts, u1 blocks, test enqueues Suspend+Resume, releases u1.
  let mailbox_for_run = Arc::clone(&mailbox);
  let run_handle = thread::spawn(move || mailbox_for_run.run(NonZeroUsize::new(10).unwrap(), None));
  entered_rx.recv().expect("runner should block inside u1 invoke");
  mailbox.enqueue_system(SystemMessage::Suspend).expect("suspend enqueue");
  mailbox.enqueue_system(SystemMessage::Resume).expect("resume enqueue");
  resume_tx.send(()).expect("release u1");

  // Then: post-user system flush drains Suspend→Resume, leaving the
  // mailbox un-suspended; u2 is then processed in the same `run()`.
  let needs_reschedule = run_handle.join().expect("run thread must join");
  assert_eq!(user_invocations.load(Ordering::SeqCst), 2, "u1 and u2 must both be invoked within a single run()");
  assert!(!mailbox.is_suspended(), "Resume must have flipped the mailbox back to un-suspended");
  assert_eq!(mailbox.user_len(), 0, "user queue must be empty after draining both u1 and u2");
  assert_eq!(mailbox.system_len(), 0, "system queue must be empty after the post-user flush");
  assert!(!needs_reschedule, "run() must return false when both queues are drained");
}

/// Test-only access to the system queue that mirrors Pekko `Mailbox.systemQueueGet()`.
/// Production code drives system drain through [`Mailbox::run`] /
/// `process_all_system_messages`; this helper is exposed only so module-local
/// tests can assert per-message contracts without installing an invoker.
impl Mailbox {
  pub(crate) fn dequeue_system(&self) -> Option<SystemMessage> {
    self.system.pop()
  }
}

/// MB-H1 follow-up: a suspended mailbox must remain schedulable while system
/// work is pending.
///
/// After MB-H1 allowed `enqueue_envelope` to accept user messages while
/// suspended, the scheduling gate had to match Pekko's
/// `Mailbox.canBeScheduledForExecution` (Mailbox.scala:148-155): when
/// suspended, the mailbox is still schedulable as long as there are system
/// messages (or a hint indicating so), otherwise `Resume` / `Terminate` could
/// never be delivered and newly accepted user messages would sit unprocessed.
#[test]
fn can_be_scheduled_for_execution_while_suspended_with_system_work() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));

  // Baseline: empty + not suspended ⇒ not schedulable without hints.
  assert!(!mailbox.can_be_scheduled_for_execution(ScheduleHints::default()));

  // When suspended and no system work exists, scheduling must still be gated.
  mailbox.suspend();
  mailbox.enqueue_user(AnyMessage::new("u1")).expect("enqueue user while suspended");
  assert!(
    !mailbox.can_be_scheduled_for_execution(ScheduleHints::default()),
    "suspended mailbox with only user work must NOT be schedulable (Pekko parity)",
  );

  // When system work is pending, the suspended mailbox must be schedulable so
  // that Resume / Terminate / Watch can be processed.
  mailbox.enqueue_system(SystemMessage::Resume).expect("enqueue system while suspended");
  assert!(
    mailbox.can_be_scheduled_for_execution(ScheduleHints::default()),
    "suspended mailbox with pending system work MUST be schedulable",
  );

  // `has_system_messages` hint alone is sufficient (Pekko contract).
  assert!(
    matches!(mailbox.dequeue_system(), Some(SystemMessage::Resume)),
    "the enqueued Resume must be the exact envelope drained here",
  );
  assert!(
    mailbox.can_be_scheduled_for_execution(ScheduleHints { has_system_messages: true, ..Default::default() }),
    "system-message hint must make a suspended mailbox schedulable",
  );

  // Resume: drain the pending user message enqueued earlier so the queue
  // is truly empty, then verify the mailbox is idle (not schedulable
  // without hints).
  mailbox.resume();
  let drained_user = mailbox.dequeue().expect("the user envelope enqueued while suspended must remain queued");
  assert_eq!(drained_user.payload().downcast_ref::<&str>().copied(), Some("u1"));
  assert!(
    !mailbox.can_be_scheduled_for_execution(ScheduleHints::default()),
    "idle resumed mailbox with empty queues must not be schedulable without hints",
  );

  // Closed: never schedulable.
  mailbox.become_closed();
  assert!(!mailbox.can_be_scheduled_for_execution(ScheduleHints {
    has_system_messages: true,
    has_user_messages:   true,
    backpressure_active: true,
  }));
}

// ---------------------------------------------------------------------------
// MB-M1: Throughput deadline enforcement tests
//
// Pekko `Mailbox.scala:261-278` regulates user-message processing by the
// conjunction of throughput (max message count) and throughput deadline
// (max elapsed time). These tests pin each branch of that contract using a
// mock clock injected via `Mailbox::set_clock`.
// ---------------------------------------------------------------------------

use core::time::Duration;

use crate::core::kernel::dispatch::mailbox::MailboxClock;

/// Test-only monotonic clock holder backed by [`SharedLock`] over
/// [`SpinSyncMutex`]. Exposes `advance()` for tests to move simulated time
/// forward and `as_mailbox_clock()` to produce a [`MailboxClock`] that reads
/// the current value.
struct MockClock {
  inner: SharedLock<Duration>,
}

impl MockClock {
  fn new(start: Duration) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<Duration>>(start) }
  }

  fn advance(&self, delta: Duration) {
    self.inner.with_write(|d| *d += delta);
  }

  fn set(&self, value: Duration) {
    self.inner.with_write(|d| *d = value);
  }

  fn as_mailbox_clock(&self) -> MailboxClock {
    let inner = self.inner.clone();
    let closure: Box<dyn Fn() -> Duration + Send + Sync> = Box::new(move || inner.with_read(|d| *d));
    ArcShared::from_boxed(closure)
  }
}

/// Invoker that advances the mock clock on each user invocation. Used by
/// tests that need `run()` to observe clock progress message-by-message.
struct AdvancingInvoker {
  user_invocations: Arc<AtomicUsize>,
  clock:            SharedLock<Duration>,
  tick:             Duration,
}

impl MessageInvoker for AdvancingInvoker {
  fn invoke(&mut self, _message: AnyMessage) -> Result<(), ActorError> {
    self.user_invocations.fetch_add(1, Ordering::SeqCst);
    self.clock.with_write(|d| *d += self.tick);
    Ok(())
  }

  fn system_invoke(&mut self, _message: SystemMessage) -> Result<(), ActorError> {
    Ok(())
  }
}

fn fill_mailbox_with_users(mailbox: &Mailbox, count: usize) {
  for i in 0..count {
    mailbox.enqueue_user(AnyMessage::new(i as u64)).expect("user enqueue should succeed");
  }
}

/// MB-M1 5.2: throughput 未消化でも deadline 超過で yield する。
#[test]
fn throughput_deadline_expired_yields_before_exhausting_throughput() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let invoker = AdvancingInvoker {
    user_invocations: user_invocations.clone(),
    clock:            mock.inner.clone(),
    tick:             Duration::from_millis(5),
  };
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(invoker)));
  fill_mailbox_with_users(&mailbox, 100);

  // deadline = 10ms, each invoke advances clock by 5ms → yield after 2 or 3 messages.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(100).unwrap(), Some(Duration::from_millis(10)));

  let processed = user_invocations.load(Ordering::SeqCst);
  assert!(processed < 100, "deadline must cause yield before exhausting throughput, got {processed} / 100",);
  assert!(processed >= 1, "at least one message must process before deadline fires");
  assert!(needs_reschedule, "unfinished work must trigger reschedule signal");
}

/// MB-M1 5.3: deadline = None では throughput を消化しきるまで続行する。
#[test]
fn throughput_deadline_none_processes_all_throughput() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let invoker = AdvancingInvoker {
    user_invocations: user_invocations.clone(),
    clock:            mock.inner.clone(),
    tick:             Duration::from_millis(5),
  };
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(invoker)));
  fill_mailbox_with_users(&mailbox, 100);

  // deadline=None: even though clock advances 5ms per invoke, run processes
  // all 100 messages (throughput-only yield behavior).
  let _ = mailbox.run(NonZeroUsize::new(100).unwrap(), None);

  assert_eq!(user_invocations.load(Ordering::SeqCst), 100, "deadline=None must allow full throughput consumption",);
}

/// MB-M1 5.4: deadline 未達で throughput 消化の場合は throughput 基準で yield する。
#[test]
fn throughput_limit_takes_precedence_when_deadline_far_in_future() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  fill_mailbox_with_users(&mailbox, 20);

  // deadline=60s (far future), throughput=10 → yield at throughput limit.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(10).unwrap(), Some(Duration::from_secs(60)));

  assert_eq!(user_invocations.load(Ordering::SeqCst), 10, "throughput=10 must cap consumption");
  assert!(needs_reschedule, "10 messages remain queued, reschedule required");
}

/// MB-M1 5.5: deadline は run() 呼び出し中ずっと一定 (ループ開始時に一度だけ計算)。
#[test]
fn deadline_computed_once_per_run() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let invoker = AdvancingInvoker {
    user_invocations: user_invocations.clone(),
    clock:            mock.inner.clone(),
    tick:             Duration::from_millis(3),
  };
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(invoker)));
  fill_mailbox_with_users(&mailbox, 50);

  // deadline = 10ms at loop start (clock=0). After ~4 invokes clock reaches 12ms,
  // which exceeds the frozen `deadline_at = 0 + 10ms = 10ms`, triggering break.
  let _ = mailbox.run(NonZeroUsize::new(50).unwrap(), Some(Duration::from_millis(10)));

  let processed = user_invocations.load(Ordering::SeqCst);
  // `deadline_at` stays at 10ms throughout the run; never recomputed against
  // the advancing clock. The exact break point depends on Pekko's `>= da`
  // semantic, but must not consume all 50 messages.
  assert!(processed < 50, "deadline is frozen at run start, clock advances should cause break (got {processed} / 50)",);
  // Lower bound confirms at least one invoke ran before deadline fire.
  assert!(processed >= 1, "at least one invoke must run");
}

/// MB-M1 5.6: monotonic clock が wall-clock 巻き戻しに耐える。
#[test]
fn monotonic_clock_resilience_to_wallclock_rewind() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  fill_mailbox_with_users(&mailbox, 5);

  // Simulate wall-clock being rewound between two run() calls. The mock clock
  // is advanced past the deadline, then reset to 0 (simulating rewind). A true
  // `Instant::now()` is monotonic and never goes backwards — the mailbox
  // code reads only `deadline_at` (a Duration computed at run start) and
  // compares against clock snapshots, so the second run() must behave
  // correctly despite the simulated rewind.
  mock.advance(Duration::from_millis(50));
  let _ = mailbox.run(NonZeroUsize::new(3).unwrap(), Some(Duration::from_millis(100)));
  // First run: deadline_at = 50 + 100 = 150ms, clock stays 50ms (CountingInvoker
  // does not advance) → all 3 throughput processed.
  assert_eq!(user_invocations.load(Ordering::SeqCst), 3, "first run processes throughput=3");

  // Simulate rewind:
  mock.set(Duration::from_millis(0));
  let _ = mailbox.run(NonZeroUsize::new(2).unwrap(), Some(Duration::from_millis(100)));
  // Second run: deadline_at = 0 + 100 = 100ms, clock stays 0ms → 2 more processed.
  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    5,
    "second run after rewind processes remaining 2 (deadline_at is recomputed per run)",
  );
}

/// MB-M1 5.7: Pekko `left > 1` 境界 — throughput=1 で deadline=ZERO でも 1 通処理。
#[test]
fn throughput_1_with_deadline_zero_yields_after_one_message() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  fill_mailbox_with_users(&mailbox, 10);

  // throughput=1, deadline=ZERO: Pekko contract = 1 message processed, then break.
  // fraktor-rs: left=1 → invoke → left=0 → while loop terminates via `left > 0`
  // (deadline break never reached; but observable outcome identical to Pekko).
  let _ = mailbox.run(NonZeroUsize::new(1).unwrap(), Some(Duration::ZERO));

  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    1,
    "throughput=1 + deadline=ZERO must process exactly 1 message (Pekko left > 1 boundary)",
  );
}

/// MB-M1 5.8: throughput=2 + deadline=ZERO + clock 固定 — deadline break 経路を踏む。
#[test]
fn throughput_2_with_deadline_zero_and_fixed_clock() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  fill_mailbox_with_users(&mailbox, 5);

  // throughput=2, deadline=ZERO, clock does not advance: deadline_at = 0.
  // After first invoke, `clock_now (=0) >= deadline_at (=0)` → break via deadline path.
  // Observable: exactly 1 invoke runs (2nd in throughput budget not consumed).
  let _ = mailbox.run(NonZeroUsize::new(2).unwrap(), Some(Duration::ZERO));

  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    1,
    "throughput=2 + deadline=ZERO + fixed clock must break after 1 message via deadline path",
  );
}

/// MB-M1 5.9: clock=None fallback — throughput-only yield behavior.
#[test]
fn clock_none_falls_back_to_throughput_only() {
  use core::num::NonZeroUsize;

  // Mailbox constructed via `Mailbox::new` gets `MailboxSharedSet::builtin()`
  // which has clock=None. No `set_clock` call → deadline enforcement disabled.
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let system_invocations = Arc::new(AtomicUsize::new(0));
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(CountingInvoker::new(
    user_invocations.clone(),
    system_invocations.clone(),
  ))));
  fill_mailbox_with_users(&mailbox, 10);

  // Even with deadline set, clock=None disables deadline enforcement.
  let _ = mailbox.run(NonZeroUsize::new(10).unwrap(), Some(Duration::from_nanos(1)));

  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    10,
    "clock=None must disable deadline enforcement (throughput-only yield)",
  );
}

/// MB-M1 5.10: throughput=10 + deadline=ZERO + clock 進行あり — 1 件処理後 break。
#[test]
fn deadline_zero_with_clock_progress_breaks_after_one_message() {
  use core::num::NonZeroUsize;

  let mut mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let mock = MockClock::new(Duration::from_millis(0));
  mailbox.set_clock(Some(mock.as_mailbox_clock()));
  let user_invocations = Arc::new(AtomicUsize::new(0));
  let invoker = AdvancingInvoker {
    user_invocations: user_invocations.clone(),
    clock:            mock.inner.clone(),
    tick:             Duration::from_micros(1),
  };
  mailbox.install_invoker(MessageInvokerShared::new(Box::new(invoker)));
  fill_mailbox_with_users(&mailbox, 10);

  // throughput=10, deadline=ZERO, invoke advances clock by 1µs:
  // deadline_at = 0 at loop start. After first invoke clock=1µs > 0 → break.
  let needs_reschedule = mailbox.run(NonZeroUsize::new(10).unwrap(), Some(Duration::ZERO));

  assert_eq!(
    user_invocations.load(Ordering::SeqCst),
    1,
    "deadline=ZERO with clock progress must break after exactly 1 message",
  );
  assert!(needs_reschedule, "9 messages remain queued, reschedule required");
}
