use alloc::vec::Vec;
use core::{hint::spin_loop, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, actor::TypedActorRef, behavior::Behavior, props::TypedProps, routers::Routers, system::TypedActorSystem,
};

#[derive(Clone)]
enum TestReq {
  Query { id: u32, reply_to: TypedActorRef<TestReply> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestReply {
  id:     u32,
  source: usize,
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

fn responding_routee_behavior(source: usize) -> Behavior<TestReq> {
  Behaviors::receive_message(move |_ctx, msg: &TestReq| {
    match msg {
      | TestReq::Query { id, reply_to } => {
        let _ = reply_to.clone().tell(TestReply { id: *id, source });
      },
    }
    Ok(Behaviors::same())
  })
}

/// fan-out 検証用: クエリ受信を共有コレクションに記録した上で応答する routee
fn tracking_routee_behavior(source: usize, tracker: ArcShared<NoStdMutex<Vec<usize>>>) -> Behavior<TestReq> {
  Behaviors::receive_message(move |_ctx, msg: &TestReq| {
    match msg {
      | TestReq::Query { id, reply_to } => {
        tracker.lock().push(source);
        let _ = reply_to.clone().tell(TestReply { id: *id, source });
      },
    }
    Ok(Behaviors::same())
  })
}

fn silent_routee_behavior() -> Behavior<TestReq> {
  Behaviors::receive_message(|_ctx, _msg: &TestReq| Ok(Behaviors::same()))
}

#[test]
fn scatter_gather_builder_builds_behavior() {
  let _behavior: Behavior<TestReq> = Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
    3,
    || responding_routee_behavior(0),
    Duration::from_secs(5),
    |msg, reply_to| match msg {
      | TestReq::Query { id, .. } => TestReq::Query { id: *id, reply_to },
    },
    |msg| match msg {
      | TestReq::Query { reply_to, .. } => Some(reply_to.clone()),
    },
    TestReply { id: 0, source: usize::MAX },
  )
  .build();
}

#[test]
#[should_panic(expected = "pool size must be positive")]
fn scatter_gather_builder_rejects_zero_pool_size() {
  let _builder = Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
    0,
    || responding_routee_behavior(0),
    Duration::from_secs(5),
    |msg, reply_to| match msg {
      | TestReq::Query { id, .. } => TestReq::Query { id: *id, reply_to },
    },
    |msg| match msg {
      | TestReq::Query { reply_to, .. } => Some(reply_to.clone()),
    },
    TestReply { id: 0, source: usize::MAX },
  );
}

#[test]
fn scatter_gather_returns_first_reply() {
  let pool_size = 3_usize;
  let next_source = ArcShared::new(NoStdMutex::new(0_usize));
  let replies = ArcShared::new(NoStdMutex::new(Vec::<TestReply>::new()));
  let replies_for_check = replies.clone();
  // fan-out 検証用: 各 routee がクエリを受信したことを記録する
  let fanout_tracker = ArcShared::new(NoStdMutex::new(Vec::<usize>::new()));
  let fanout_for_check = fanout_tracker.clone();

  let ns = next_source.clone();
  let ft = fanout_tracker.clone();
  let routee_factory = ArcShared::new(move || -> Behavior<TestReq> {
    let source = {
      let mut guard = ns.lock();
      let s = *guard;
      *guard += 1;
      s
    };
    tracking_routee_behavior(source, ft.clone())
  });

  let replies_for_collector = replies.clone();
  let collector_factory = ArcShared::new(move || -> Behavior<TestReply> {
    let rr = replies_for_collector.clone();
    Behaviors::receive_message(move |_ctx, msg: &TestReply| {
      rr.lock().push(msg.clone());
      Ok(Behaviors::same())
    })
  });

  let rf = routee_factory.clone();
  let cf = collector_factory.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<TestReq> {
    let cf = cf.clone();
    let rf = rf.clone();
    Behaviors::setup(move |ctx| {
      let cf2 = cf.clone();
      let collector_props = TypedProps::<TestReply>::from_behavior_factory(move || cf2());
      let collector_child = ctx.spawn_child(&collector_props).expect("collector");
      let collector_ref = collector_child.actor_ref().clone();

      let rf2 = rf.clone();
      let router_behavior_factory = ArcShared::new(move || {
        Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          {
            let rf3 = rf2.clone();
            move || rf3()
          },
          Duration::from_secs(5),
          |msg, reply_to| match msg {
            | TestReq::Query { id, .. } => TestReq::Query { id: *id, reply_to },
          },
          |msg| match msg {
            | TestReq::Query { reply_to, .. } => Some(reply_to.clone()),
          },
          TestReply { id: 0, source: usize::MAX },
        )
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<TestReq>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &TestReq| {
        match msg {
          | TestReq::Query { id, .. } => {
            let _ = router_ref.clone().tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
          },
        }
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<TestReq>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  let dummy_reply_to = TypedActorRef::from_untyped(crate::core::actor::actor_ref::ActorRef::no_sender());
  guardian.tell(TestReq::Query { id: 42, reply_to: dummy_reply_to }).expect("tell");

  wait_until(|| !replies_for_check.lock().is_empty());

  // 全 routee がクエリを受信したことを検証 (fan-out)
  wait_until(|| fanout_for_check.lock().len() == pool_size);
  let tracked = fanout_for_check.lock();
  assert_eq!(tracked.len(), pool_size, "scatter-gather は全 routee にファンアウトすべき");

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1, "scatter-gather should return exactly one reply");
  assert_eq!(got[0].id, 42);
  assert!(got[0].source < pool_size, "source should be a valid routee index");

  system.terminate().expect("terminate");
}

#[test]
fn scatter_gather_returns_timeout_reply_when_no_routee_responds() {
  let pool_size = 2_usize;
  let replies = ArcShared::new(NoStdMutex::new(Vec::<TestReply>::new()));
  let replies_for_check = replies.clone();
  let timeout_reply = TestReply { id: 0, source: usize::MAX };

  let replies_for_collector = replies.clone();
  let collector_factory = ArcShared::new(move || -> Behavior<TestReply> {
    let rr = replies_for_collector.clone();
    Behaviors::receive_message(move |_ctx, msg: &TestReply| {
      rr.lock().push(msg.clone());
      Ok(Behaviors::same())
    })
  });

  let cf = collector_factory.clone();
  let tr = timeout_reply.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<TestReq> {
    let cf = cf.clone();
    let tr = tr.clone();
    Behaviors::setup(move |ctx| {
      let cf2 = cf.clone();
      let collector_props = TypedProps::<TestReply>::from_behavior_factory(move || cf2());
      let collector_child = ctx.spawn_child(&collector_props).expect("collector");
      let collector_ref = collector_child.actor_ref().clone();

      let tr2 = tr.clone();
      let router_behavior_factory = ArcShared::new(move || {
        Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          silent_routee_behavior,
          Duration::from_millis(10),
          |msg, reply_to| match msg {
            | TestReq::Query { id, .. } => TestReq::Query { id: *id, reply_to },
          },
          |msg| match msg {
            | TestReq::Query { reply_to, .. } => Some(reply_to.clone()),
          },
          tr2.clone(),
        )
        .build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<TestReq>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");
      let router_ref = router_child.actor_ref().clone();

      Behaviors::receive_message(move |_ctx, msg: &TestReq| {
        match msg {
          | TestReq::Query { id, .. } => {
            let _ = router_ref.clone().tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
          },
        }
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let manual_driver = crate::core::scheduler::tick_driver::ManualTestDriver::new();
  let controller = manual_driver.controller();
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(manual_driver);
  let system = TypedActorSystem::<TestReq>::new(&props, tick_driver).expect("system");
  let mut guardian = system.user_guardian_ref();

  let dummy_reply_to = TypedActorRef::from_untyped(crate::core::actor::actor_ref::ActorRef::no_sender());
  guardian.tell(TestReq::Query { id: 99, reply_to: dummy_reply_to }).expect("tell");

  // メッセージ伝播を待ち、スケジューラを駆動してタイムアウトを発火させる
  for _ in 0..20 {
    spin_loop();
  }
  controller.inject_and_drive(200);

  wait_until(|| !replies_for_check.lock().is_empty());

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1);
  assert_eq!(got[0], timeout_reply);

  system.terminate().expect("terminate");
}
