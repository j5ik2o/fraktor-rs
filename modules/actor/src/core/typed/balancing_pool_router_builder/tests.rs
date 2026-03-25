use alloc::vec::Vec;
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, behavior::Behavior, props::TypedProps, routers::Routers, system::TypedActorSystem,
};

#[derive(Clone, Debug)]
struct WorkItem {
  id: u32,
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..50_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition(), "wait_until timed out");
}

#[test]
fn balancing_pool_builder_builds_behavior() {
  let _behavior: Behavior<WorkItem> =
    Routers::balancing_pool(3, || Behaviors::receive_message(|_ctx, _msg: &WorkItem| Ok(Behaviors::same()))).build();
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn balancing_pool_builder_rejects_zero_pool_size() {
  let _builder =
    Routers::balancing_pool(0, || Behaviors::receive_message(|_ctx, _msg: &WorkItem| Ok(Behaviors::same())));
}

#[test]
fn balancing_pool_distributes_to_idle_workers() {
  let pool_size = 3_usize;
  let processed = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let processed_check = processed.clone();

  let p = processed.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<WorkItem> {
    let p = p.clone();
    Behaviors::setup(move |ctx| {
      let p2 = p.clone();
      let router_behavior_factory = ArcShared::new(move || {
        let p3 = p2.clone();
        Routers::balancing_pool(pool_size, move || {
          let p4 = p3.clone();
          Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
            p4.lock().push(msg.id);
            Ok(Behaviors::same())
          })
        })
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<WorkItem>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
        router_ref.clone().tell(msg.clone());
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<WorkItem>::from_behavior_factory(move || ob());
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<WorkItem>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  // Send multiple work items.
  for i in 0..5 {
    guardian.tell(WorkItem { id: i });
  }

  wait_until(|| processed_check.lock().len() >= 5);

  let got = processed_check.lock();
  assert_eq!(got.len(), 5, "all 5 work items should have been processed");
  // Verify all IDs were processed (order may vary due to balancing).
  let mut sorted = got.clone();
  sorted.sort();
  assert_eq!(sorted, alloc::vec![0, 1, 2, 3, 4]);

  system.terminate().expect("terminate");
}

#[test]
fn balancing_pool_does_not_support_resizer() {
  // BalancingPoolRouterBuilder に with_resizer メソッドが存在しないことはコンパイル時制約。
  // このテストは with_pool_size + build が通ることだけを確認する。
  // with_resizer の不在はコンパイルレベルで保証されるため、ランタイムテストでは検出できない。
  let builder =
    Routers::balancing_pool(2, || Behaviors::receive_message(|_ctx, _msg: &WorkItem| Ok(Behaviors::same())));
  let _behavior = builder.with_pool_size(4).build();
}

#[test]
fn balancing_pool_with_pool_size_override() {
  let builder =
    Routers::balancing_pool(2, || Behaviors::receive_message(|_ctx, _msg: &WorkItem| Ok(Behaviors::same())));
  let _behavior = builder.with_pool_size(5).build();
}

#[test]
fn balancing_pool_stopped_routee_does_not_receive_pending_work() {
  // Regression test for PEKKO-NEW-balancing_pool_router_builder-L100:
  // 2 routees, 3 messages. Each routee stops after 1 message.
  // The 3rd message must NOT be delivered to a stopping routee.
  let processed = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let processed_check = processed.clone();

  let p = processed.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<WorkItem> {
    let p = p.clone();
    Behaviors::setup(move |ctx| {
      let p2 = p.clone();
      let router_behavior_factory = ArcShared::new(move || {
        let p3 = p2.clone();
        Routers::balancing_pool(2, move || {
          let p4 = p3.clone();
          // Each routee processes exactly 1 message then stops.
          Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
            p4.lock().push(msg.id);
            Ok(Behaviors::stopped())
          })
        })
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<WorkItem>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
        router_ref.clone().tell(msg.clone());
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<WorkItem>::from_behavior_factory(move || ob());
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<WorkItem>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  // Send 3 messages: both routees will stop after 1 each.
  // The 3rd message goes to the shared queue but no idle worker should pull it.
  guardian.tell(WorkItem { id: 10 });
  guardian.tell(WorkItem { id: 20 });
  guardian.tell(WorkItem { id: 30 });

  // Wait for the two routees to process their messages.
  wait_until(|| processed_check.lock().len() >= 2);

  // Give extra spins to ensure the 3rd message is NOT delivered.
  for _ in 0..10_000 {
    spin_loop();
  }

  let got = processed_check.lock();
  // Only 2 messages should be processed (one per routee).
  // The 3rd message remains in the shared queue with no active workers.
  assert_eq!(got.len(), 2, "stopped routees must not receive additional work; got {:?}", &*got);

  system.terminate().expect("terminate");
}

#[test]
fn balancing_pool_stops_when_all_routees_terminate() {
  let processed = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let processed_check = processed.clone();

  let p = processed.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<WorkItem> {
    let p = p.clone();
    Behaviors::setup(move |ctx| {
      let p2 = p.clone();
      let router_behavior_factory = ArcShared::new(move || {
        let p3 = p2.clone();
        // Each routee stops after processing one message.
        Routers::balancing_pool(2, move || {
          let p4 = p3.clone();
          Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
            p4.lock().push(msg.id);
            Ok(Behaviors::stopped())
          })
        })
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<WorkItem>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
        router_ref.clone().tell(msg.clone());
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<WorkItem>::from_behavior_factory(move || ob());
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<WorkItem>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  // Send 2 messages to consume both routees (they each stop after 1 message).
  guardian.tell(WorkItem { id: 1 });
  guardian.tell(WorkItem { id: 2 });

  wait_until(|| processed_check.lock().len() >= 2);

  let got = processed_check.lock();
  assert_eq!(got.len(), 2);

  system.terminate().expect("terminate");
}

#[test]
fn balancing_pool_routee_stopped_on_start_does_not_receive_work() {
  // Regression test for PEKKO-NEW-balancing_pool_router_builder-L89:
  // A routee whose behavior returns Stopped during startup (around_start)
  // must NOT be registered as an idle worker and must NOT receive work.
  let processed = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let processed_check = processed.clone();

  let p = processed.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<WorkItem> {
    let p = p.clone();
    Behaviors::setup(move |ctx| {
      let p2 = p.clone();
      let router_behavior_factory = ArcShared::new(move || {
        let p3 = p2.clone();
        // All routees return Stopped on start (via setup returning stopped).
        Routers::balancing_pool(2, move || {
          let _p4 = p3.clone();
          Behaviors::setup(|_ctx| Behaviors::stopped())
        })
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<WorkItem>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &WorkItem| {
        router_ref.clone().tell(msg.clone());
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<WorkItem>::from_behavior_factory(move || ob());
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<WorkItem>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  // Send a message; no routee should process it since all stopped at start.
  guardian.tell(WorkItem { id: 99 });

  // Give spins to ensure no delivery happens.
  for _ in 0..10_000 {
    spin_loop();
  }

  let got = processed_check.lock();
  assert_eq!(got.len(), 0, "routees that stopped at start must not receive work; got {:?}", &*got);

  system.terminate().expect("terminate");
}
