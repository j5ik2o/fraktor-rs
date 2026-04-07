use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::super::{broadcast::Broadcast, routee::Routee, router::Router, routing_logic::RoutingLogic};
use crate::core::kernel::actor::{
  Pid,
  actor_ref::{ActorRef, ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A [`RoutingLogic`] that always selects the routee at the given index.
struct FixedIndexLogic(usize);

impl RoutingLogic for FixedIndexLogic {
  fn select<'a>(&self, _message: &AnyMessage, routees: &'a [Routee]) -> &'a Routee {
    &routees[self.0]
  }
}

/// A sender that records delivered messages.
struct CountingSender {
  count:    ArcShared<AtomicUsize>,
  messages: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
}

impl ActorRefSender for CountingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.count.fetch_add(1, Ordering::Relaxed);
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct ClosedSender;

impl ActorRefSender for ClosedSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

fn make_counting_routee(
  pid: Pid,
  counter: &ArcShared<AtomicUsize>,
  messages: &ArcShared<NoStdMutex<Vec<AnyMessage>>>,
) -> Routee {
  let sender = CountingSender { count: counter.clone(), messages: messages.clone() };
  Routee::ActorRef(ActorRef::new(pid, sender))
}

fn routee_pids(routees: &[Routee]) -> Vec<Pid> {
  routees
    .iter()
    .filter_map(|routee| match routee {
      | Routee::ActorRef(actor_ref) => Some(actor_ref.pid()),
      | _ => None,
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_creates_router_with_logic_and_routees() {
  // Given: a logic and two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];

  // When: creating a new Router
  let router = Router::new(FixedIndexLogic(0), routees);

  // Then: routees() returns the initial routees
  assert_eq!(router.routees().len(), 2);
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

#[test]
fn route_sends_to_selected_routee() {
  // Given: a router with two routees, logic selects index 1
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];
  let mut router = Router::new(FixedIndexLogic(1), routees);

  // When: routing a normal message
  let result = router.route(AnyMessage::new(42_u32));

  // Then: only routee at index 1 receives the message
  assert!(result.is_ok());
  assert_eq!(c0.load(Ordering::Relaxed), 0);
  assert_eq!(c1.load(Ordering::Relaxed), 1);
}

#[test]
fn route_broadcast_sends_to_all_routees() {
  // Given: a router with three routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m2 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![
    make_counting_routee(Pid::new(1, 0), &c0, &m0),
    make_counting_routee(Pid::new(2, 0), &c1, &m1),
    make_counting_routee(Pid::new(3, 0), &c2, &m2),
  ];
  let mut router = Router::new(FixedIndexLogic(0), routees);

  // When: routing a Broadcast message
  let result = router.route(AnyMessage::new(Broadcast(AnyMessage::new(99_u32))));

  // Then: all routees receive the message
  assert!(result.is_ok());
  assert_eq!(c0.load(Ordering::Relaxed), 1);
  assert_eq!(c1.load(Ordering::Relaxed), 1);
  assert_eq!(c2.load(Ordering::Relaxed), 1);
  for messages in [&m0, &m1, &m2] {
    let deliveries = messages.lock();
    assert_eq!(deliveries.len(), 1);
    let payload =
      deliveries[0].downcast_ref::<u32>().expect("each routee should receive the unwrapped broadcast payload");
    assert_eq!(*payload, 99_u32);
  }
}

#[test]
fn route_broadcast_continues_after_first_send_error() {
  // Given: a router whose first routee is closed and remaining routees are healthy
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m2 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![
    Routee::ActorRef(ActorRef::new(Pid::new(1, 0), ClosedSender)),
    make_counting_routee(Pid::new(2, 0), &c1, &m1),
    make_counting_routee(Pid::new(3, 0), &c2, &m2),
  ];
  let mut router = Router::new(FixedIndexLogic(0), routees);

  // When: routing a Broadcast message
  let result = router.route(AnyMessage::new(Broadcast(AnyMessage::new(99_u32))));

  // Then: the first error is returned, but later routees still receive the inner payload
  assert!(result.is_err(), "broadcast should surface the first send error");
  assert_eq!(c1.load(Ordering::Relaxed), 1);
  assert_eq!(c2.load(Ordering::Relaxed), 1);
  for messages in [&m1, &m2] {
    let deliveries = messages.lock();
    assert_eq!(deliveries.len(), 1);
    let payload =
      deliveries[0].downcast_ref::<u32>().expect("each healthy routee should receive the unwrapped broadcast payload");
    assert_eq!(*payload, 99_u32);
  }
}

#[test]
fn route_with_no_routees_returns_ok() {
  // Given: a router with no routees
  let mut router = Router::new(FixedIndexLogic(0), vec![]);

  // When: routing a message
  let result = router.route(AnyMessage::new(1_u32));

  // Then: returns Ok (no panic, message is dropped)
  assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Immutable updates
// ---------------------------------------------------------------------------

#[test]
fn with_routees_replaces_all_routees() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: replacing with a new set of three routees
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let c4 = ArcShared::new(AtomicUsize::new(0));
  let c5 = ArcShared::new(AtomicUsize::new(0));
  let m3 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m4 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m5 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let new_routees = vec![
    make_counting_routee(Pid::new(10, 0), &c3, &m3),
    make_counting_routee(Pid::new(11, 0), &c4, &m4),
    make_counting_routee(Pid::new(12, 0), &c5, &m5),
  ];
  let router = router.with_routees(new_routees);

  // Then: 置換後の routee 群と順序が一致する
  assert_eq!(routee_pids(router.routees()), vec![Pid::new(10, 0), Pid::new(11, 0), Pid::new(12, 0)]);
}

#[test]
fn add_routee_appends_to_list() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: adding a third routee
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let m2 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let new_routee = make_counting_routee(Pid::new(3, 0), &c2, &m2);
  let router = router.add_routee(new_routee);

  // Then: 末尾に追加される
  assert_eq!(routee_pids(router.routees()), vec![Pid::new(1, 0), Pid::new(2, 0), Pid::new(3, 0)]);
}

#[test]
fn remove_routee_removes_matching() {
  // Given: a router with three routees (same pid for comparison)
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m2 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![
    make_counting_routee(Pid::new(1, 0), &c0, &m0),
    make_counting_routee(Pid::new(2, 0), &c1, &m1),
    make_counting_routee(Pid::new(3, 0), &c2, &m2),
  ];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: removing the middle routee by creating one with the same pid
  let c_ref = ArcShared::new(AtomicUsize::new(0));
  let m_ref = ArcShared::new(NoStdMutex::new(Vec::new()));
  let to_remove = make_counting_routee(Pid::new(2, 0), &c_ref, &m_ref);
  let router = router.remove_routee(&to_remove);

  // Then: 指定した pid の routee だけが削除される
  assert_eq!(routee_pids(router.routees()), vec![Pid::new(1, 0), Pid::new(3, 0)]);
}

#[test]
fn remove_routee_with_no_match_keeps_all() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: removing a routee that does not exist in the list
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let m3 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let non_existent = make_counting_routee(Pid::new(99, 0), &c3, &m3);
  let router = router.remove_routee(&non_existent);

  // Then: routee 群は変わらない
  assert_eq!(routee_pids(router.routees()), vec![Pid::new(1, 0), Pid::new(2, 0)]);
}

// ---------------------------------------------------------------------------
// Accessor
// ---------------------------------------------------------------------------

#[test]
fn routees_accessor_returns_correct_len() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let m1 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0), make_counting_routee(Pid::new(2, 0), &c1, &m1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: accessing routees
  let slice = router.routees();

  // Then: the slice has correct length
  assert_eq!(slice.len(), 2);
}

// ---------------------------------------------------------------------------
// NoRoutee selection
// ---------------------------------------------------------------------------

#[test]
fn route_with_noroutee_selected_returns_ok() {
  // Given: a router with routees but logic returns a static NoRoutee
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let m0 = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0, &m0)];

  struct NoRouteeLogic;

  impl RoutingLogic for NoRouteeLogic {
    fn select<'a>(&self, _message: &AnyMessage, _routees: &'a [Routee]) -> &'a Routee {
      static NOROUTEE: Routee = Routee::NoRoutee;
      &NOROUTEE
    }
  }

  let mut router = Router::new(NoRouteeLogic, routees);

  // When: routing a normal message
  let result = router.route(AnyMessage::new(42_u32));

  // Then: returns Ok and no routee receives a message
  assert!(result.is_ok());
  assert_eq!(c0.load(Ordering::Relaxed), 0);
}
