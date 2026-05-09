use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  super::{deafen::Deafen, listen::Listen, with_listeners::WithListeners},
  *,
};
use crate::actor::{
  Pid,
  actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// 受信メッセージを記録する送信器。
struct CountingSender {
  count:    ArcShared<AtomicUsize>,
  messages: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl ActorRefSender for CountingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.count.fetch_add(1, Ordering::Relaxed);
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

// 常に Closed エラーを返す送信器。
struct ClosedSender;

impl ActorRefSender for ClosedSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

fn make_null_ref(id: u64) -> ActorRef {
  ActorRef::new_with_builtin_lock(Pid::new(id, 0), NullSender)
}

fn make_counting_ref(
  id: u64,
  counter: &ArcShared<AtomicUsize>,
  messages: &ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
) -> ActorRef {
  let sender = CountingSender { count: counter.clone(), messages: messages.clone() };
  ActorRef::new_with_builtin_lock(Pid::new(id, 0), sender)
}

fn make_closed_ref(id: u64) -> ActorRef {
  ActorRef::new_with_builtin_lock(Pid::new(id, 0), ClosedSender)
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_creates_empty_listeners() {
  let listeners = Listeners::new();

  assert!(listeners.is_empty());
  assert_eq!(listeners.len(), 0);
}

#[test]
fn default_equals_new() {
  let listeners = Listeners::default();

  assert!(listeners.is_empty());
  assert_eq!(listeners.len(), 0);
}

// ---------------------------------------------------------------------------
// handle: Listen
// ---------------------------------------------------------------------------

#[test]
fn handle_listen_adds_actor_ref() {
  let mut listeners = Listeners::new();
  let listener = make_null_ref(1);
  let message = AnyMessage::new(Listen(listener));

  let handled = listeners.handle(&message.as_view());

  assert!(handled);
  assert_eq!(listeners.len(), 1);
  assert!(!listeners.is_empty());
}

#[test]
fn handle_listen_with_duplicate_pid_is_idempotent() {
  let mut listeners = Listeners::new();
  let first = make_null_ref(7);
  let second = make_null_ref(7);

  let first_msg = AnyMessage::new(Listen(first));
  let second_msg = AnyMessage::new(Listen(second));

  assert!(listeners.handle(&first_msg.as_view()));
  assert!(listeners.handle(&second_msg.as_view()));

  assert_eq!(listeners.len(), 1);
}

#[test]
fn handle_listen_with_distinct_pids_adds_each() {
  let mut listeners = Listeners::new();

  for id in [10_u64, 20, 30] {
    let msg = AnyMessage::new(Listen(make_null_ref(id)));
    assert!(listeners.handle(&msg.as_view()));
  }

  assert_eq!(listeners.len(), 3);
}

// ---------------------------------------------------------------------------
// handle: Deafen
// ---------------------------------------------------------------------------

#[test]
fn handle_deafen_removes_matching_listener() {
  let mut listeners = Listeners::new();
  let target = make_null_ref(1);
  let add_msg = AnyMessage::new(Listen(target.clone()));
  assert!(listeners.handle(&add_msg.as_view()));
  assert_eq!(listeners.len(), 1);

  let remove_msg = AnyMessage::new(Deafen(target));
  let handled = listeners.handle(&remove_msg.as_view());

  assert!(handled);
  assert_eq!(listeners.len(), 0);
  assert!(listeners.is_empty());
}

#[test]
fn handle_deafen_on_missing_pid_is_noop_but_handled() {
  let mut listeners = Listeners::new();
  let stranger = make_null_ref(42);
  let remove_msg = AnyMessage::new(Deafen(stranger));

  let handled = listeners.handle(&remove_msg.as_view());

  assert!(handled);
  assert_eq!(listeners.len(), 0);
}

#[test]
fn handle_deafen_removes_only_matching_pid() {
  let mut listeners = Listeners::new();
  for id in [1_u64, 2, 3] {
    let msg = AnyMessage::new(Listen(make_null_ref(id)));
    assert!(listeners.handle(&msg.as_view()));
  }
  assert_eq!(listeners.len(), 3);

  let remove_msg = AnyMessage::new(Deafen(make_null_ref(2)));
  assert!(listeners.handle(&remove_msg.as_view()));

  assert_eq!(listeners.len(), 2);
}

// ---------------------------------------------------------------------------
// handle: WithListeners
// ---------------------------------------------------------------------------

#[test]
fn handle_with_listeners_invokes_callback_for_each_registered_listener() {
  let mut listeners = Listeners::new();
  let pids = [Pid::new(11, 0), Pid::new(22, 0), Pid::new(33, 0)];
  for pid in pids {
    let msg = AnyMessage::new(Listen(ActorRef::new_with_builtin_lock(pid, NullSender)));
    assert!(listeners.handle(&msg.as_view()));
  }

  let visited = ArcShared::new(SpinSyncMutex::new(Vec::<Pid>::new()));
  let visited_clone = visited.clone();
  let with = WithListeners::new(move |actor_ref: &ActorRef| {
    visited_clone.lock().push(actor_ref.pid());
  });
  let msg = AnyMessage::new(with);
  let handled = listeners.handle(&msg.as_view());

  assert!(handled);
  let collected = visited.lock().clone();
  assert_eq!(collected, vec![Pid::new(11, 0), Pid::new(22, 0), Pid::new(33, 0)]);
}

#[test]
fn handle_with_listeners_on_empty_invokes_callback_zero_times() {
  let mut listeners = Listeners::new();

  let visited = ArcShared::new(AtomicUsize::new(0));
  let visited_clone = visited.clone();
  let with = WithListeners::new(move |_actor_ref: &ActorRef| {
    visited_clone.fetch_add(1, Ordering::Relaxed);
  });
  let msg = AnyMessage::new(with);
  let handled = listeners.handle(&msg.as_view());

  assert!(handled);
  assert_eq!(visited.load(Ordering::Relaxed), 0);
}

// ---------------------------------------------------------------------------
// handle: non-listener messages
// ---------------------------------------------------------------------------

#[test]
fn handle_returns_false_for_unrelated_message() {
  let mut listeners = Listeners::new();
  let unrelated = AnyMessage::new(123_u32);

  let handled = listeners.handle(&unrelated.as_view());

  assert!(!handled);
  assert_eq!(listeners.len(), 0);
}

#[test]
fn handle_returns_false_for_unrelated_message_with_registered_listeners() {
  let mut listeners = Listeners::new();
  let listen = AnyMessage::new(Listen(make_null_ref(1)));
  assert!(listeners.handle(&listen.as_view()));
  let unrelated = AnyMessage::new("hello");

  let handled = listeners.handle(&unrelated.as_view());

  assert!(!handled);
  assert_eq!(listeners.len(), 1);
}

// ---------------------------------------------------------------------------
// gossip
// ---------------------------------------------------------------------------

#[test]
fn gossip_delivers_to_all_listeners_and_returns_ok() {
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let m1 = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let m2 = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let m3 = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let mut listeners = Listeners::new();
  for (id, counter, msgs) in [(1_u64, &c1, &m1), (2, &c2, &m2), (3, &c3, &m3)] {
    let msg = AnyMessage::new(Listen(make_counting_ref(id, counter, msgs)));
    assert!(listeners.handle(&msg.as_view()));
  }

  let result = listeners.gossip(AnyMessage::new(42_u32));

  assert!(result.is_ok());
  assert_eq!(c1.load(Ordering::Relaxed), 1);
  assert_eq!(c2.load(Ordering::Relaxed), 1);
  assert_eq!(c3.load(Ordering::Relaxed), 1);
  for messages in [&m1, &m2, &m3] {
    let deliveries = messages.lock();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].downcast_ref::<u32>(), Some(&42_u32));
  }
}

#[test]
fn gossip_on_empty_returns_ok() {
  let mut listeners = Listeners::new();

  let result = listeners.gossip(AnyMessage::new(1_u32));

  assert!(result.is_ok());
}

#[test]
fn gossip_continues_after_first_send_error_and_returns_it() {
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let m2 = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let m3 = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let mut listeners = Listeners::new();
  {
    let msg = AnyMessage::new(Listen(make_closed_ref(1)));
    assert!(listeners.handle(&msg.as_view()));
  }
  {
    let msg = AnyMessage::new(Listen(make_counting_ref(2, &c2, &m2)));
    assert!(listeners.handle(&msg.as_view()));
  }
  {
    let msg = AnyMessage::new(Listen(make_counting_ref(3, &c3, &m3)));
    assert!(listeners.handle(&msg.as_view()));
  }

  let result = listeners.gossip(AnyMessage::new(99_u32));

  // Then: first-error を Err として返しつつ、後続リスナーには配送される
  assert!(result.is_err(), "gossip should surface the first send error");
  assert_eq!(c2.load(Ordering::Relaxed), 1);
  assert_eq!(c3.load(Ordering::Relaxed), 1);
  for messages in [&m2, &m3] {
    let deliveries = messages.lock();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].downcast_ref::<u32>(), Some(&99_u32));
  }
}
