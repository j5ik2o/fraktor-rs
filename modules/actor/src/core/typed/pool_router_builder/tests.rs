use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::{pseudo_random_index, select_smallest_mailbox_index};
use crate::core::{
  actor::{Actor, ActorCell, ActorContextGeneric, Pid},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::ActorSystem,
  typed::{
    Behaviors, actor::TypedActorRef, behavior::Behavior, props::TypedPropsGeneric, routers::Routers,
    system::TypedActorSystemGeneric,
  },
};

type RouteRecord = (usize, u32);
type RouterSystemContext =
  (TypedActorSystemGeneric<u32, NoStdToolbox>, TypedActorRef<u32>, ArcShared<NoStdMutex<Vec<RouteRecord>>>);

#[derive(Clone, Copy)]
enum PoolTestStrategy {
  Broadcast,
  Random { seed: u64 },
  ConsistentHash,
}

struct IdleActor;

impl Actor for IdleActor {
  fn receive(
    &mut self,
    _context: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
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

fn recording_routee_behavior(
  routee_index: usize,
  records: ArcShared<NoStdMutex<Vec<RouteRecord>>>,
) -> Behavior<u32, NoStdToolbox> {
  Behaviors::receive_message(move |_ctx, message| {
    records.lock().push((routee_index, *message));
    Ok(Behaviors::same())
  })
}

fn spawn_router_system(pool_size: usize, strategy: PoolTestStrategy) -> RouterSystemContext {
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let next_routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let props = TypedPropsGeneric::<u32, NoStdToolbox>::from_behavior_factory({
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
      let builder = Routers::pool::<u32, NoStdToolbox, _>(pool_size, routee_factory);
      let builder = match strategy {
        | PoolTestStrategy::Broadcast => builder.with_broadcast(),
        | PoolTestStrategy::Random { seed } => builder.with_random(seed),
        | PoolTestStrategy::ConsistentHash => builder.with_consistent_hash(|message| *message as u64),
      };
      builder.build()
    }
  });

  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystemGeneric::<u32, NoStdToolbox>::new(&props, tick_driver).expect("system");
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
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_pool_size_override() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_pool_size(5);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_broadcast_builds_behavior() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_broadcast();
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_random_builds_behavior() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_random(42);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_consistent_hash_builds_behavior() {
  let builder =
    Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_consistent_hash(|message| *message as u64);
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_smallest_mailbox_builds_behavior() {
  let builder = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_smallest_mailbox();
  let _behavior: Behavior<u32, NoStdToolbox> = builder.build();
}

#[test]
fn pool_router_builder_with_broadcast_delivers_to_all_routees() {
  let pool_size = 3_usize;
  let (system, mut router, records) = spawn_router_system(pool_size, PoolTestStrategy::Broadcast);

  router.tell(11).expect("tell");
  wait_until(|| records.lock().len() == pool_size);

  let mut routees: Vec<usize> =
    records.lock().iter().filter_map(|(routee_index, message)| (*message == 11).then_some(*routee_index)).collect();
  routees.sort_unstable();
  assert_eq!(routees, vec![0, 1, 2]);

  system.terminate().expect("terminate");
}

#[test]
fn pool_router_builder_with_random_routes_reproducibly_from_seed() {
  let seed = 42_u64;
  let pool_size = 3_usize;
  let message_count = 9_usize;
  let (system, mut router, records) = spawn_router_system(pool_size, PoolTestStrategy::Random { seed });

  for message in 0..message_count {
    router.tell(message as u32).expect("tell");
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
    router.tell(message).expect("tell");
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
  let _builder = Routers::pool::<u32, NoStdToolbox, _>(0, Behaviors::ignore);
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn pool_router_builder_with_pool_size_rejects_zero() {
  let _ = Routers::pool::<u32, NoStdToolbox, _>(3, Behaviors::ignore).with_pool_size(0);
}
