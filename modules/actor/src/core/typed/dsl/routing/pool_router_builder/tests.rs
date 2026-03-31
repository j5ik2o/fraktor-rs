use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::{pseudo_random_index, select_smallest_mailbox_index};
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    system::ActorSystem,
  },
  typed::{
    TypedActorRef,
    behavior::Behavior,
    dsl::{
      Behaviors,
      routing::{DefaultResizer, Routers},
    },
    props::TypedProps,
    system::TypedActorSystem,
  },
};

type RouteRecord = (usize, u32);
type RouterSystemContext = (TypedActorSystem<u32>, TypedActorRef<u32>, ArcShared<NoStdMutex<Vec<RouteRecord>>>);

#[derive(Clone, Copy)]
enum PoolTestStrategy {
  Broadcast,
  Random { seed: u64 },
  ConsistentHash,
}

struct IdleActor;

impl Actor for IdleActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

fn recording_routee_behavior(routee_index: usize, records: ArcShared<NoStdMutex<Vec<RouteRecord>>>) -> Behavior<u32> {
  Behaviors::receive_message(move |_ctx, message| {
    records.lock().push((routee_index, *message));
    Ok(Behaviors::same())
  })
}

fn spawn_router_system(pool_size: usize, strategy: PoolTestStrategy) -> RouterSystemContext {
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    move || {
      let routee_factory = {
        let records = records.clone();
        let next_routee_index = next_routee_index.clone();
        move || {
          let routee_index = {
            let mut guard = next_routee_index.lock();
            let current = *guard;
            *guard += 1;
            current
          };
          recording_routee_behavior(routee_index, records.clone())
        }
      };
      let builder = Routers::pool::<u32, _>(pool_size, routee_factory);
      let builder = match strategy {
        | PoolTestStrategy::Broadcast => builder.with_broadcast(),
        | PoolTestStrategy::Random { seed } => builder.with_random(seed),
        | PoolTestStrategy::ConsistentHash => builder.with_consistent_hash(|message| *message as u64),
      };
      builder.build()
    }
  });

  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&props, tick_driver).expect("system");
  let router = system.user_guardian_ref();
  (system, router, records)
}

fn register_routee_cell(system: &ActorSystem, pid: Pid, name: &str) -> ArcShared<ActorCell> {
  let props = Props::from_fn(|| IdleActor);
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), &props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

#[test]
fn pool_router_builder_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_pool_size_override() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_pool_size(5);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_broadcast_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_broadcast();
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_round_robin_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_round_robin();
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_random_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_random(42);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_consistent_hash_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_consistent_hash(|message| *message as u64);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_smallest_mailbox_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_smallest_mailbox();
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_broadcast_predicate_builds_behavior() {
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_broadcast_predicate(|message| *message == 0);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_broadcast_delivers_to_all_routees() {
  let pool_size = 3_usize;
  let (system, mut router, records) = spawn_router_system(pool_size, PoolTestStrategy::Broadcast);

  router.tell(11);
  wait_until(|| records.lock().len() == pool_size);

  let mut routees: Vec<usize> =
    records.lock().iter().filter_map(|(routee_index, message)| (*message == 11).then_some(*routee_index)).collect();
  routees.sort_unstable();
  assert_eq!(routees, vec![0, 1, 2]);

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_broadcast_predicate_only_broadcasts_matching_messages() {
  let pool_size = 3_usize;
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    move || {
      let routee_factory = {
        let records = records.clone();
        let next_routee_index = next_routee_index.clone();
        move || {
          let routee_index = {
            let mut guard = next_routee_index.lock();
            let current = *guard;
            *guard += 1;
            current
          };
          recording_routee_behavior(routee_index, records.clone())
        }
      };
      Routers::pool::<u32, _>(pool_size, routee_factory).with_broadcast_predicate(|message| *message == 99).build()
    }
  });

  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&props, tick_driver).expect("system");
  let mut router = system.user_guardian_ref();

  router.tell(7);
  wait_until(|| records.lock().len() == 1);
  assert_eq!(records.lock().iter().filter(|(_, message)| *message == 7).count(), 1);

  router.tell(99);
  wait_until(|| records.lock().iter().filter(|(_, message)| *message == 99).count() == pool_size);

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_random_routes_reproducibly_from_seed() {
  let seed = 42_u64;
  let pool_size = 3_usize;
  let message_count = 9_usize;
  let (system, mut router, records) = spawn_router_system(pool_size, PoolTestStrategy::Random { seed });

  for message in 0..message_count {
    router.tell(message as u32);
  }
  wait_until(|| records.lock().len() == message_count);

  let mut routee_by_message = vec![usize::MAX; message_count];
  for (routee_index, message) in records.lock().iter().copied() {
    let slot = &mut routee_by_message[message as usize];
    assert_eq!(*slot, usize::MAX, "message routed more than once");
    *slot = routee_index;
  }

  for (message, routee_index) in routee_by_message.into_iter().enumerate() {
    let expected = pseudo_random_index((message as u64) ^ seed, pool_size);
    assert_eq!(routee_index, expected);
  }

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_consistent_hash_routes_to_hash_bucket() {
  let pool_size = 3_usize;
  let messages = [0_u32, 3, 1, 4, 2, 5, 0, 3, 1];
  let (system, mut router, records) = spawn_router_system(pool_size, PoolTestStrategy::ConsistentHash);

  for message in messages {
    router.tell(message);
  }
  wait_until(|| records.lock().len() == messages.len());

  for (routee_index, message) in records.lock().iter().copied() {
    assert_eq!(routee_index, (message as usize) % pool_size);
  }

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_smallest_mailbox_selects_lowest_queue() {
  let system = ActorSystem::new_empty();

  let pid0 = system.allocate_pid();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  let cell0 = register_routee_cell(&system, pid0, "routee-0");
  let cell1 = register_routee_cell(&system, pid1, "routee-1");
  let cell2 = register_routee_cell(&system, pid2, "routee-2");

  cell0.mailbox().enqueue_user(AnyMessage::new(1_u32)).expect("enqueue");
  cell0.mailbox().enqueue_user(AnyMessage::new(2_u32)).expect("enqueue");
  cell1.mailbox().enqueue_user(AnyMessage::new(3_u32)).expect("enqueue");

  let routees = vec![
    TypedActorRef::<u32>::from_untyped(cell0.actor_ref()),
    TypedActorRef::<u32>::from_untyped(cell1.actor_ref()),
    TypedActorRef::<u32>::from_untyped(cell2.actor_ref()),
  ];
  let dispatch_counts = ArcShared::new(NoStdMutex::new(vec![0_usize; routees.len()]));

  let selected = select_smallest_mailbox_index(&routees, &dispatch_counts);
  assert_eq!(selected, 2);
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn pool_router_builder_rejects_zero_pool_size() {
  let _builder = Routers::pool::<u32, _>(0, Behaviors::ignore);
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn pool_router_builder_with_pool_size_rejects_zero() {
  let _ = Routers::pool::<u32, _>(3, Behaviors::ignore).with_pool_size(0);
}

#[test]
fn pool_router_builder_with_resizer_builds_behavior() {
  let resizer = DefaultResizer::new(2, 5, 1);
  let builder = Routers::pool::<u32, _>(3, Behaviors::ignore).with_resizer(resizer);
  let _behavior: Behavior<u32> = builder.build();
}

#[test]
fn pool_router_builder_with_resizer_scales_up_to_lower_bound() {
  let initial_pool_size = 2_usize;
  let lower_bound = 4_usize;
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    move || {
      let routee_factory = {
        let records = records.clone();
        let next_routee_index = next_routee_index.clone();
        move || {
          let routee_index = {
            let mut guard = next_routee_index.lock();
            let current = *guard;
            *guard += 1;
            current
          };
          recording_routee_behavior(routee_index, records.clone())
        }
      };
      // 初期2台、resizer下限4 ⇒ 最初のメッセージでさらに2台追加される
      let resizer = DefaultResizer::new(lower_bound, 10, 1);
      Routers::pool::<u32, _>(initial_pool_size, routee_factory).with_resizer(resizer).build()
    }
  });

  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&props, tick_driver).expect("system");
  let mut router = system.user_guardian_ref();

  // ラウンドロビンで全routeeを使い切るのに十分なメッセージを送信
  for msg in 0..lower_bound as u32 {
    router.tell(msg);
  }
  wait_until(|| records.lock().len() == lower_bound);

  // スケールアップ後に生成された routee 数が lower_bound と一致することを検証
  assert_eq!(*next_routee_index.lock(), lower_bound, "スケールアップ後の生成 routee 数が lower_bound と一致するべき",);

  // メッセージを受信したrouteeのユニークなインデックスを収集
  let mut seen_routees: Vec<usize> = records.lock().iter().map(|(idx, _)| *idx).collect();
  seen_routees.sort_unstable();
  seen_routees.dedup();
  assert_eq!(seen_routees.len(), lower_bound);

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_resizer_scales_down_to_upper_bound() {
  let initial_pool_size = 5_usize;
  let upper_bound = 3_usize;
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    let next_routee_index = next_routee_index.clone();
    move || {
      let routee_factory = {
        let records = records.clone();
        let next_routee_index = next_routee_index.clone();
        move || {
          let routee_index = {
            let mut guard = next_routee_index.lock();
            let current = *guard;
            *guard += 1;
            current
          };
          recording_routee_behavior(routee_index, records.clone())
        }
      };
      // 初期5台、resizer上限3 ⇒ 最初のメッセージで2台削除される
      let resizer = DefaultResizer::new(1, upper_bound, 1);
      Routers::pool::<u32, _>(initial_pool_size, routee_factory).with_resizer(resizer).build()
    }
  });

  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&props, tick_driver).expect("system");
  let mut router = system.user_guardian_ref();

  // 残存する全routeeを巡回するのに十分なメッセージを送信
  let message_count = upper_bound * 2;
  for msg in 0..message_count as u32 {
    router.tell(msg);
  }
  wait_until(|| records.lock().len() == message_count);

  // リサイズ縮小後、routee 0..upper_bound のみがメッセージを受信するべき
  // routee 3, 4（初期spawn分）は停止されているはず
  let mut seen_routees: Vec<usize> = records.lock().iter().map(|(idx, _)| *idx).collect();
  seen_routees.sort_unstable();
  seen_routees.dedup();
  assert_eq!(
    seen_routees.len(),
    upper_bound,
    "expected exactly {} unique routees after resize-down, got {}",
    upper_bound,
    seen_routees.len()
  );

  system.terminate().expect("terminate");
}
