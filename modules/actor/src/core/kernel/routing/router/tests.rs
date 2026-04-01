use alloc::vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::SendError,
    messaging::AnyMessage,
  },
  routing::{Broadcast, Routee, Router, RoutingLogic},
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

/// A sender that records how many messages were received.
struct CountingSender {
  count: ArcShared<AtomicUsize>,
}

impl ActorRefSender for CountingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(SendOutcome::Delivered)
  }
}

fn make_counting_routee(pid: Pid, counter: &ArcShared<AtomicUsize>) -> Routee {
  let sender = CountingSender { count: counter.clone() };
  Routee::ActorRef(ActorRef::new(pid, sender))
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_creates_router_with_logic_and_routees() {
  // Given: a logic and two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];

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
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];
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
  let routees = vec![
    make_counting_routee(Pid::new(1, 0), &c0),
    make_counting_routee(Pid::new(2, 0), &c1),
    make_counting_routee(Pid::new(3, 0), &c2),
  ];
  let mut router = Router::new(FixedIndexLogic(0), routees);

  // When: routing a Broadcast message
  let result = router.route(AnyMessage::new(Broadcast(AnyMessage::new(99_u32))));

  // Then: all routees receive the message
  assert!(result.is_ok());
  assert_eq!(c0.load(Ordering::Relaxed), 1);
  assert_eq!(c1.load(Ordering::Relaxed), 1);
  assert_eq!(c2.load(Ordering::Relaxed), 1);
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
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: replacing with a new set of three routees
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let c4 = ArcShared::new(AtomicUsize::new(0));
  let c5 = ArcShared::new(AtomicUsize::new(0));
  let new_routees = vec![
    make_counting_routee(Pid::new(10, 0), &c3),
    make_counting_routee(Pid::new(11, 0), &c4),
    make_counting_routee(Pid::new(12, 0), &c5),
  ];
  let router = router.with_routees(new_routees);

  // Then: the router has three new routees
  assert_eq!(router.routees().len(), 3);
}

#[test]
fn add_routee_appends_to_list() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: adding a third routee
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let new_routee = make_counting_routee(Pid::new(3, 0), &c2);
  let router = router.add_routee(new_routee);

  // Then: the router has three routees
  assert_eq!(router.routees().len(), 3);
}

#[test]
fn remove_routee_removes_matching() {
  // Given: a router with three routees (same pid for comparison)
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let c2 = ArcShared::new(AtomicUsize::new(0));
  let routees = vec![
    make_counting_routee(Pid::new(1, 0), &c0),
    make_counting_routee(Pid::new(2, 0), &c1),
    make_counting_routee(Pid::new(3, 0), &c2),
  ];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: removing the middle routee by creating one with the same pid
  let c_ref = ArcShared::new(AtomicUsize::new(0));
  let to_remove = make_counting_routee(Pid::new(2, 0), &c_ref);
  let router = router.remove_routee(&to_remove);

  // Then: the router has two remaining routees
  assert_eq!(router.routees().len(), 2);
}

#[test]
fn remove_routee_with_no_match_keeps_all() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];
  let router = Router::new(FixedIndexLogic(0), routees);

  // When: removing a routee that does not exist in the list
  let c3 = ArcShared::new(AtomicUsize::new(0));
  let non_existent = make_counting_routee(Pid::new(99, 0), &c3);
  let router = router.remove_routee(&non_existent);

  // Then: all routees remain
  assert_eq!(router.routees().len(), 2);
}

// ---------------------------------------------------------------------------
// Accessor
// ---------------------------------------------------------------------------

#[test]
fn routees_accessor_returns_correct_len() {
  // Given: a router with two routees
  let c0 = ArcShared::new(AtomicUsize::new(0));
  let c1 = ArcShared::new(AtomicUsize::new(0));
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0), make_counting_routee(Pid::new(2, 0), &c1)];
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
  let routees = vec![make_counting_routee(Pid::new(1, 0), &c0)];

  struct NoRouteeLogic;

  impl RoutingLogic for NoRouteeLogic {
    fn select<'a>(&self, _message: &AnyMessage, _routees: &'a [Routee]) -> &'a Routee {
      static NOROUTEE: std::sync::OnceLock<Routee> = std::sync::OnceLock::new();
      NOROUTEE.get_or_init(|| Routee::NoRoutee)
    }
  }

  let mut router = Router::new(NoRouteeLogic, routees);

  // When: routing a normal message
  let result = router.route(AnyMessage::new(42_u32));

  // Then: returns Ok and no routee receives a message
  assert!(result.is_ok());
  assert_eq!(c0.load(Ordering::Relaxed), 0);
}
