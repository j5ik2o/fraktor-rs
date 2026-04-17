use alloc::string::String;

use super::super::{
  routee::Routee, routing_logic::RoutingLogic, smallest_mailbox_routing_logic::SmallestMailboxRoutingLogic,
};
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, NullSender},
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::ActorSystem,
};

struct IdleActor;

impl Actor for IdleActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn register_routee(system: &ActorSystem, pid: Pid, name: &str) -> Routee {
  let props = Props::from_fn(|| IdleActor);
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), &props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  Routee::ActorRef(cell.actor_ref())
}

fn standalone_routee(pid: Pid) -> Routee {
  Routee::ActorRef(ActorRef::new_with_builtin_lock(pid, NullSender))
}

#[test]
fn new_creates_logic() {
  // Given/When
  let _logic = SmallestMailboxRoutingLogic::new();

  // Then
  // construction succeeds without panic
}

#[test]
fn select_empty_routees_returns_no_routee() {
  // Given
  let logic = SmallestMailboxRoutingLogic::new();
  let routees: [Routee; 0] = [];
  let message = AnyMessage::new(1_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  assert!(matches!(selected, Routee::NoRoutee));
}

#[test]
fn select_prefers_routee_with_smallest_observed_mailbox() {
  // Given
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "routee-0");
  let routee1 = register_routee(&system, pid1, "routee-1");
  let routee2 = register_routee(&system, pid2, "routee-2");
  let routees = [routee0, routee1, routee2];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(10_u32);

  let cell0 = system.state().cell(&pid0).expect("cell-0");
  let cell1 = system.state().cell(&pid1).expect("cell-1");
  cell0.mailbox().enqueue_user(AnyMessage::new(1_u32)).expect("enqueue");
  cell0.mailbox().enqueue_user(AnyMessage::new(2_u32)).expect("enqueue");
  cell1.mailbox().enqueue_user(AnyMessage::new(3_u32)).expect("enqueue");

  // When
  let selected = logic.select(&message, &routees);

  // Then
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid2),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_prefers_observable_routee_over_unobservable_routees() {
  // Given
  let system = ActorSystem::new_empty();
  let observable_pid = system.allocate_pid();
  let observable = register_routee(&system, observable_pid, "observable");
  let routees = [standalone_routee(Pid::new(90, 0)), observable, standalone_routee(Pid::new(91, 0))];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(11_u32);

  let observable_cell = system.state().cell(&observable_pid).expect("observable cell");
  observable_cell.mailbox().enqueue_user(AnyMessage::new(99_u32)).expect("enqueue");

  // When
  let selected = logic.select(&message, &routees);

  // Then
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), observable_pid),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}
