use alloc::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};

use fraktor_utils_core_rs::core::sync::RuntimeMutex;

use crate::core::kernel::{
  actor::{
    Pid,
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
  },
  dispatch::mailbox::{
    DequeMessageQueue, Envelope, Mailbox, MailboxInstrumentation, MailboxMessage, MailboxOverflowStrategy,
    MailboxPolicy, MessageQueue, UnboundedDequeMessageQueue,
  },
  system::ActorSystem,
};

enum ScriptedEnqueue {
  Enqueued,
  Full,
  Closed,
}

struct ScriptedMessageQueue {
  messages:  RuntimeMutex<VecDeque<Envelope>>,
  outcomes:  RuntimeMutex<VecDeque<ScriptedEnqueue>>,
  full_hook: RuntimeMutex<Option<ScriptedFullHook>>,
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
      messages:  RuntimeMutex::new(VecDeque::new()),
      outcomes:  RuntimeMutex::new(outcomes),
      full_hook: RuntimeMutex::new(full_hook),
    }
  }
}

impl MessageQueue for ScriptedMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let outcome = self.outcomes.lock().pop_front().expect("enqueue outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.lock().push_back(envelope);
        Ok(())
      },
      | ScriptedEnqueue::Full => {
        if let Some(hook) = self.full_hook.lock().take() {
          hook.before_error_tx.send(()).expect("full hook notification must be delivered");
          hook.resume_rx.recv().expect("full hook resume signal must be delivered");
        }
        Err(SendError::full(envelope.into_payload()))
      },
      | ScriptedEnqueue::Closed => Err(SendError::closed(envelope.into_payload())),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.messages.lock().pop_front()
  }

  fn number_of_messages(&self) -> usize {
    self.messages.lock().len()
  }

  fn clean_up(&self) {
    self.messages.lock().clear();
  }
}

struct ScriptedDequeMessageQueue {
  messages:                RuntimeMutex<VecDeque<Envelope>>,
  enqueue_outcomes:        RuntimeMutex<VecDeque<ScriptedEnqueue>>,
  enqueue_first_outcomes:  RuntimeMutex<VecDeque<ScriptedEnqueue>>,
  enqueue_first_full_hook: RuntimeMutex<Option<ScriptedFullHook>>,
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
      messages:                RuntimeMutex::new(VecDeque::new()),
      enqueue_outcomes:        RuntimeMutex::new(enqueue_outcomes),
      enqueue_first_outcomes:  RuntimeMutex::new(enqueue_first_outcomes),
      enqueue_first_full_hook: RuntimeMutex::new(enqueue_first_full_hook),
    }
  }
}

impl MessageQueue for ScriptedDequeMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let outcome = self.enqueue_outcomes.lock().pop_front().expect("enqueue outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.lock().push_back(envelope);
        Ok(())
      },
      | ScriptedEnqueue::Full => Err(SendError::full(envelope.into_payload())),
      | ScriptedEnqueue::Closed => Err(SendError::closed(envelope.into_payload())),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.messages.lock().pop_front()
  }

  fn number_of_messages(&self) -> usize {
    self.messages.lock().len()
  }

  fn clean_up(&self) {
    self.messages.lock().clear();
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    Some(self)
  }
}

impl DequeMessageQueue for ScriptedDequeMessageQueue {
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError> {
    let outcome = self.enqueue_first_outcomes.lock().pop_front().expect("enqueue_first outcome must be configured");
    match outcome {
      | ScriptedEnqueue::Enqueued => {
        self.messages.lock().push_front(envelope);
        Ok(())
      },
      | ScriptedEnqueue::Full => {
        if let Some(hook) = self.enqueue_first_full_hook.lock().take() {
          hook.before_error_tx.send(()).expect("full hook notification must be delivered");
          hook.resume_rx.recv().expect("full hook resume signal must be delivered");
        }
        Err(SendError::full(envelope.into_payload()))
      },
      | ScriptedEnqueue::Closed => Err(SendError::closed(envelope.into_payload())),
    }
  }
}

fn expect_next_user_message(mailbox: &Mailbox, expected: &str) {
  let Some(MailboxMessage::User(envelope)) = mailbox.dequeue() else {
    panic!("user message expected");
  };
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

#[test]
fn mailbox_enqueue_user_suspended() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.suspend();
  let message = AnyMessage::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_err());
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
  let result = mailbox.dequeue();
  assert!(result.is_none());
}

#[test]
fn mailbox_dequeue_user_message() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  let result = mailbox.dequeue();
  assert!(result.is_some());
}

#[test]
fn mailbox_dequeue_system_message_priority() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let user_message = AnyMessage::new(1_u32);
  mailbox.enqueue_user(user_message).unwrap();
  let system_message = SystemMessage::Stop;
  mailbox.enqueue_system(system_message).unwrap();

  let result = mailbox.dequeue();
  assert!(result.is_some());
  if let Some(msg) = result {
    assert!(matches!(msg, crate::core::kernel::dispatch::mailbox::MailboxMessage::System(_)));
  }
}

#[test]
fn mailbox_dequeue_suspended() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  mailbox.suspend();
  let result = mailbox.dequeue();
  assert!(result.is_none());
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
  assert!(mailbox.dequeue().is_some());
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
  mailbox.become_closed_and_clean_up();
  assert!(mailbox.is_closed());
}

#[test]
fn mailbox_enqueue_envelope_returns_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.become_closed_and_clean_up();

  let result = mailbox.enqueue_envelope(Envelope::new(AnyMessage::new("msg")));
  assert!(matches!(result, Err(SendError::Closed(_))), "expected Closed, got {result:?}");
}

#[test]
fn mailbox_enqueue_user_returns_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.become_closed_and_clean_up();

  let result = mailbox.enqueue_user(AnyMessage::new("msg"));
  assert!(matches!(result, Err(SendError::Closed(_))), "expected Closed, got {result:?}");
}

#[test]
fn mailbox_prepend_user_messages_deque_returns_closed_after_mailbox_close() {
  let queue = Box::new(UnboundedDequeMessageQueue::new());
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);
  mailbox.become_closed_and_clean_up();

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
/// The test thread takes `user_queue_lock`, starts a producer, waits until the
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
  let guard = mailbox.user_queue_lock.lock();
  let (started_tx, started_rx) = mpsc::channel();
  let (result_tx, result_rx) = mpsc::channel();
  let mailbox_for_enqueue = Arc::clone(&mailbox);
  let enqueue_handle = thread::spawn(move || {
    started_tx.send(()).expect("enqueue 開始シグナルが送信されるべき");
    let result = mailbox_for_enqueue.enqueue_user(AnyMessage::new("inflight"));
    result_tx.send(result).expect("enqueue 結果が送信されるべき");
  });

  started_rx.recv().expect("enqueue スレッドが起動するべき");
  assert!(
    result_rx.recv_timeout(Duration::from_millis(200)).is_err(),
    "producer は user_queue_lock 上でブロックされるべき",
  );

  mailbox.state.close();
  mailbox.user.clean_up();
  drop(guard);

  let result = result_rx.recv().expect("enqueue 結果を受信できるべき");
  enqueue_handle.join().expect("enqueue スレッドが完了するべき");
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
  let guard = mailbox.user_queue_lock.lock();
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
  assert!(
    result_rx.recv_timeout(Duration::from_millis(200)).is_err(),
    "prepend は user_queue_lock 上でブロックされるべき",
  );

  mailbox.state.close();
  mailbox.user.clean_up();
  drop(guard);

  let result = result_rx.recv().expect("prepend 結果を受信できるべき");
  prepend_handle.join().expect("prepend スレッドが完了するべき");
  assert!(
    matches!(result, Err(SendError::Closed(_))),
    "under-lock re-check must reject inflight prepend, got {result:?}",
  );
  assert_eq!(mailbox.user_len(), 0, "no phantom prepended message should be in the queue");
}
