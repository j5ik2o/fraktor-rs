use alloc::string::String;

use super::super::{
  routee::Routee, routing_logic::RoutingLogic, smallest_mailbox_routing_logic::SmallestMailboxRoutingLogic,
};
use crate::{
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

#[test]
fn select_prefers_idle_over_processing() {
  // Given: 2 routees, both empty mailbox, but routee0 is processing (is_running == true)
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "processing");
  let routee1 = register_routee(&system, pid1, "idle");
  let routees = [routee0, routee1];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(20_u32);

  let cell0 = system.state().cell(&pid0).expect("cell-0");
  cell0.mailbox().set_running();

  // When
  let selected = logic.select(&message, &routees);

  // Then: idle routee1 wins because processing routee0 gets score 1 (processing penalty)
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid1),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_suspended_routee_is_last_resort() {
  // Given: routee0 is suspended, routee1 has 5 messages, routee2 is unobservable
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "suspended");
  let routee1 = register_routee(&system, pid1, "has-messages");
  let routee2 = standalone_routee(Pid::new(92, 0));
  let routees = [routee0, routee1, routee2];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(21_u32);

  let cell0 = system.state().cell(&pid0).expect("cell-0");
  cell0.mailbox().suspend();
  let cell1 = system.state().cell(&pid1).expect("cell-1");
  for i in 0..5_u32 {
    cell1.mailbox().enqueue_user(AnyMessage::new(i)).expect("enqueue");
  }

  // When
  let selected = logic.select(&message, &routees);

  // Then: routee1 wins (5 messages < SCORE_UNKNOWN < SCORE_SUSPENDED)
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid1),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_score_zero_short_circuits_on_first_idle_empty() {
  // Given: 3 idle+empty routees. Per Pekko's short-circuit, first (routee0) wins.
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "idle-0");
  let routee1 = register_routee(&system, pid1, "idle-1");
  let routee2 = register_routee(&system, pid2, "idle-2");
  let routees = [routee0, routee1, routee2];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(22_u32);

  // When
  let selected = logic.select(&message, &routees);

  // Then
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid0),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_prefers_processing_empty_over_idle_with_messages_regardless_of_index_order() {
  // Pekko 仕様: empty mailbox は has_messages より優先される
  // (documented priority 6 > 5)。ペナルティ込みの deep score が同値になる
  // (A: processing+empty=1, B: idle+1msg=1) 場合でも、empty mailbox 側が
  // 勝つことを保証する。
  //
  // routee の順序に関係なく A が選ばれることを確認するため、A を index 1 に
  // 配置する（index 0 に B を配置）。pass 1 の shallow score で A=1 が B=MAX-3
  // を下回るため best=A となり、pass 2 でも引き継がれて A が返る。
  let system = ActorSystem::new_empty();
  let pid_b = system.allocate_pid();
  let pid_a = system.allocate_pid();
  let routee_b = register_routee(&system, pid_b, "idle-1msg");
  let routee_a = register_routee(&system, pid_a, "processing-empty");
  // 順序: B (index 0), A (index 1)
  let routees = [routee_b, routee_a];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(30_u32);

  let cell_b = system.state().cell(&pid_b).expect("cell-b");
  cell_b.mailbox().enqueue_user(AnyMessage::new(777_u32)).expect("enqueue");
  let cell_a = system.state().cell(&pid_a).expect("cell-a");
  cell_a.mailbox().set_running();

  // When
  let selected = logic.select(&message, &routees);

  // Then: A (processing+empty, index 1) が Pekko 仕様で勝つ。
  // 1 パス目追跡を欠く実装では B (idle+1msg, index 0) が勝ってしまう。
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid_a),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_prefers_idle_with_messages_over_processing_with_fewer_messages_of_same_penalty() {
  // Pekko 準拠: 処理中ペナルティ(+1) は件数にも加算される。
  // routee0: idle + 3 msgs → score = 0 + 3 = 3
  // routee1: processing + 2 msgs → score = 1 + 2 = 3 (tie)
  // routee2: processing + 1 msg → score = 1 + 1 = 2 ← 最小
  // => 2 ルーティーが同点ならば最初に見つかった routee が勝つ（Pekko 仕様: `<` 比較）
  //    今回は routee2 が唯一の score=2 のため選ばれる
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "idle-3");
  let routee1 = register_routee(&system, pid1, "processing-2");
  let routee2 = register_routee(&system, pid2, "processing-1");
  let routees = [routee0, routee1, routee2];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(24_u32);

  let cell0 = system.state().cell(&pid0).expect("cell-0");
  let cell1 = system.state().cell(&pid1).expect("cell-1");
  let cell2 = system.state().cell(&pid2).expect("cell-2");
  for i in 0..3_u32 {
    cell0.mailbox().enqueue_user(AnyMessage::new(i)).expect("enqueue");
  }
  for i in 0..2_u32 {
    cell1.mailbox().enqueue_user(AnyMessage::new(i + 10)).expect("enqueue");
  }
  cell1.mailbox().set_running();
  cell2.mailbox().enqueue_user(AnyMessage::new(100_u32)).expect("enqueue");
  cell2.mailbox().set_running();

  // When
  let selected = logic.select(&message, &routees);

  // Then: routee2 (processing + 1 msg, score 2) wins over routee0 (score 3) and routee1 (score 3)
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid2),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}

#[test]
fn select_prefers_fewest_messages_on_deep_pass() {
  // Given: all routees have at least 1 message. Second pass picks lowest count.
  let system = ActorSystem::new_empty();
  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  let routee0 = register_routee(&system, pid0, "many");
  let routee1 = register_routee(&system, pid1, "few");
  let routee2 = register_routee(&system, pid2, "medium");
  let routees = [routee0, routee1, routee2];
  let logic = SmallestMailboxRoutingLogic::new();
  let message = AnyMessage::new(23_u32);

  let cell0 = system.state().cell(&pid0).expect("cell-0");
  let cell1 = system.state().cell(&pid1).expect("cell-1");
  let cell2 = system.state().cell(&pid2).expect("cell-2");
  for i in 0..5_u32 {
    cell0.mailbox().enqueue_user(AnyMessage::new(i)).expect("enqueue");
  }
  cell1.mailbox().enqueue_user(AnyMessage::new(100_u32)).expect("enqueue");
  for i in 0..3_u32 {
    cell2.mailbox().enqueue_user(AnyMessage::new(i + 10)).expect("enqueue");
  }

  // When
  let selected = logic.select(&message, &routees);

  // Then: routee1 (1 message) wins
  match selected {
    | Routee::ActorRef(actor_ref) => assert_eq!(actor_ref.pid(), pid1),
    | Routee::NoRoutee | Routee::Several(_) => panic!("expected ActorRef routee"),
  }
}
