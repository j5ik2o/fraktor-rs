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
    Envelope, Mailbox, MailboxInstrumentation, MailboxOverflowStrategy, MailboxPolicy, MessageQueue,
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
fn mailbox_prepend_user_messages_returns_error_and_restores_existing_only() {
  let outcomes = VecDeque::from([
    ScriptedEnqueue::Enqueued, // existing-1
    ScriptedEnqueue::Enqueued, // existing-2
    ScriptedEnqueue::Enqueued, // prepend（drain 後の再エンキュー）
    ScriptedEnqueue::Full,     // existing-1 の再エンキューが失敗
    // clean_up 後、既存メッセージのみ復元
    ScriptedEnqueue::Enqueued, // 復元: existing-1
    ScriptedEnqueue::Enqueued, // 復元: existing-2
  ]);
  let queue = Box::new(ScriptedMessageQueue::new(outcomes));
  let mailbox = Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue);

  mailbox.enqueue_user(AnyMessage::new("existing-1")).expect("existing-1");
  mailbox.enqueue_user(AnyMessage::new("existing-2")).expect("existing-2");

  let prepended = VecDeque::from([AnyMessage::new("prepend")]);
  let result = mailbox.prepend_user_messages(&prepended);

  // 新メッセージの挿入は失敗として報告される
  assert!(result.is_err(), "prepend should fail: {result:?}");
}

#[test]
fn mailbox_prepend_user_messages_blocks_concurrent_enqueue_until_prepend_finishes() {
  use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  let outcomes = VecDeque::from([
    ScriptedEnqueue::Enqueued, // existing-1
    ScriptedEnqueue::Enqueued, // existing-2
    ScriptedEnqueue::Enqueued, // prepend（drain-and-requeue 1回目）
    ScriptedEnqueue::Full,     // existing-1 の再エンキューが失敗 → ロールバック発動
    // 既存メッセージのみ復元（新メッセージは含めない）
    ScriptedEnqueue::Enqueued, // 復元: existing-1
    ScriptedEnqueue::Enqueued, // 復元: existing-2
    ScriptedEnqueue::Enqueued, // 並行エンキュー
  ]);
  let (before_error_tx, before_error_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let queue = Box::new(ScriptedMessageQueue::new_with_full_hook(
    outcomes,
    Some(ScriptedFullHook::new(before_error_tx, resume_rx)),
  ));
  let mailbox = Arc::new(Mailbox::new_with_queue(MailboxPolicy::unbounded(None), queue));

  mailbox.enqueue_user(AnyMessage::new("existing-1")).expect("existing-1");
  mailbox.enqueue_user(AnyMessage::new("existing-2")).expect("existing-2");

  let mailbox_for_prepend = Arc::clone(&mailbox);
  let prepended = VecDeque::from([AnyMessage::new("prepend")]);
  let prepend_handle = thread::spawn(move || mailbox_for_prepend.prepend_user_messages(&prepended));

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
  // existing-1(復元) + existing-2(復元) + concurrent = 3
  assert_eq!(mailbox.user_len(), 3);
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
fn mailbox_prepend_user_messages_returns_closed_after_mailbox_close() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  mailbox.become_closed_and_clean_up();

  let messages = VecDeque::from([AnyMessage::new("msg")]);
  let result = mailbox.prepend_user_messages(&messages);
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
/// exercising the `prepend_user_messages` path used by
/// `ActorCell::unstash_*`.
#[test]
fn cleanup_close_wins_against_inflight_prepend() {
  use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
  };

  let mailbox = Arc::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let guard = mailbox.user_queue_lock.lock();
  let messages = VecDeque::from([AnyMessage::new("inflight-prepend")]);
  let (started_tx, started_rx) = mpsc::channel();
  let (result_tx, result_rx) = mpsc::channel();
  let mailbox_for_prepend = Arc::clone(&mailbox);
  let prepend_handle = thread::spawn(move || {
    started_tx.send(()).expect("prepend 開始シグナルが送信されるべき");
    let result = mailbox_for_prepend.prepend_user_messages(&messages);
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
