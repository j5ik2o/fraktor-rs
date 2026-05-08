extern crate std;

use alloc::vec::Vec;
use core::time::Duration;
use std::time::Instant;

use fraktor_actor_core_rs::core::kernel::actor::{actor_ref::ActorRef, setup::ActorSystemConfig};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::{
  TypedActorRef,
  behavior::Behavior,
  dsl::{Behaviors, routing::Routers},
  props::TypedProps,
  system::TypedActorSystem,
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
  let deadline = Instant::now() + Duration::from_secs(1);
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    std::thread::yield_now();
  }
  assert!(condition(), "wait_until timed out");
}

fn responding_routee_behavior(source: usize) -> Behavior<TestReq> {
  Behaviors::receive_message(move |_ctx, msg: &TestReq| {
    match msg {
      | TestReq::Query { id, reply_to } => {
        reply_to.clone().tell(TestReply { id: *id, source });
      },
    }
    Ok(Behaviors::same())
  })
}

// fan-out 検証用: クエリ受信を共有コレクションに記録した上で応答する routee
fn tracking_routee_behavior(source: usize, tracker: ArcShared<SpinSyncMutex<Vec<usize>>>) -> Behavior<TestReq> {
  Behaviors::receive_message(move |_ctx, msg: &TestReq| {
    match msg {
      | TestReq::Query { id, reply_to } => {
        tracker.lock().push(source);
        reply_to.clone().tell(TestReply { id: *id, source });
      },
    }
    Ok(Behaviors::same())
  })
}

fn silent_routee_behavior() -> Behavior<TestReq> {
  Behaviors::receive_message(|_ctx, _msg: &TestReq| Ok(Behaviors::same()))
}

/// Routee that immediately stops on creation (to simulate all-routee termination).
fn immediately_stopping_routee_behavior() -> Behavior<TestReq> {
  Behaviors::stopped()
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
  let next_source = ArcShared::new(SpinSyncMutex::new(0_usize));
  let replies = ArcShared::new(SpinSyncMutex::new(Vec::<TestReply>::new()));
  let replies_for_check = replies.clone();
  // fan-out 検証用: 各 routee がクエリを受信したことを記録する
  let fanout_tracker = ArcShared::new(SpinSyncMutex::new(Vec::<usize>::new()));
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
      let collector_ref = collector_child.into_actor_ref();

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

      Behaviors::receive_message(move |_ctx, msg: &TestReq| {
        match msg {
          | TestReq::Query { id, .. } => {
            let mut router = router_child.clone();
            router.tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
          },
        }
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let system = TypedActorSystem::<TestReq>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut guardian = system.user_guardian_ref();

  let dummy_reply_to = TypedActorRef::from_untyped(ActorRef::no_sender());
  guardian.tell(TestReq::Query { id: 42, reply_to: dummy_reply_to });

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
  let replies = ArcShared::new(SpinSyncMutex::new(Vec::<TestReply>::new()));
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
      let collector_ref = collector_child.into_actor_ref();

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

      Behaviors::receive_message(move |_ctx, msg: &TestReq| {
        match msg {
          | TestReq::Query { id, .. } => {
            let mut router = router_child.clone();
            router.tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
          },
        }
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let system = TypedActorSystem::<TestReq>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut guardian = system.user_guardian_ref();

  let dummy_reply_to = TypedActorRef::from_untyped(ActorRef::no_sender());
  guardian.tell(TestReq::Query { id: 99, reply_to: dummy_reply_to });

  // TestTickDriver が自動でティックを駆動するのでタイムアウト発火を待つ
  wait_until(|| !replies_for_check.lock().is_empty());

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1);
  assert_eq!(got[0], timeout_reply);

  system.terminate().expect("terminate");
}

/// Regression: When all routees terminate via Terminated signal, the router
/// should transition to `stopped()` (empty routee list triggers stop).
#[test]
fn scatter_gather_stops_when_all_routees_terminate() {
  let pool_size = 2_usize;
  let router_stopped = ArcShared::new(SpinSyncMutex::new(false));
  let router_stopped_check = router_stopped.clone();

  let rs = router_stopped.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<TestReq> {
    let rs = rs.clone();
    Behaviors::setup(move |ctx| {
      let router_behavior_factory = ArcShared::new(move || {
        Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          immediately_stopping_routee_behavior,
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
      let _router_child = ctx.spawn_child_watched(&router_props).expect("router");

      let rs2 = rs.clone();
      Behaviors::receive_message(move |_ctx, _msg: &TestReq| Ok(Behaviors::same())).receive_signal(
        move |_ctx, signal| {
          use crate::message_and_signals::BehaviorSignal;
          match signal {
            | BehaviorSignal::Terminated(_) => {
              *rs2.lock() = true;
              Ok(Behaviors::same())
            },
            | _ => Ok(Behaviors::same()),
          }
        },
      )
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let system = TypedActorSystem::<TestReq>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");

  // routee が即座に停止するため、Terminated シグナルでルーターも停止する
  wait_until(|| *router_stopped_check.lock());

  system.terminate().expect("terminate");
}

/// Regression: When all routee spawns fail during build, the router should
/// immediately transition to `Behaviors::stopped()` via the empty-routee guard.
#[test]
fn scatter_gather_stops_when_all_routee_spawns_fail() {
  let pool_size = 3_usize;
  let router_stopped = ArcShared::new(SpinSyncMutex::new(false));
  let router_stopped_check = router_stopped.clone();

  let rs = router_stopped.clone();
  let outer_behavior = ArcShared::new(move || -> Behavior<TestReq> {
    let rs = rs.clone();
    Behaviors::setup(move |ctx| {
      let router_behavior_factory = ArcShared::new(move || {
        let mut builder = Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
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
        builder.force_routee_spawn_failure = true;
        builder.build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<TestReq>::from_behavior_factory(move || rbf());
      let _router_child = ctx.spawn_child_watched(&router_props).expect("router");

      let rs2 = rs.clone();
      Behaviors::receive_message(move |_ctx, _msg: &TestReq| Ok(Behaviors::same())).receive_signal(
        move |_ctx, signal| {
          use crate::message_and_signals::BehaviorSignal;
          match signal {
            | BehaviorSignal::Terminated(_) => {
              *rs2.lock() = true;
              Ok(Behaviors::same())
            },
            | _ => Ok(Behaviors::same()),
          }
        },
      )
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let system = TypedActorSystem::<TestReq>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");

  // spawn 失敗により routee_vec が空 → build 直後に Behaviors::stopped() → ルーター終了
  wait_until(|| *router_stopped_check.lock());

  system.terminate().expect("terminate");
}

/// Regression: When coordinator spawn fails, the caller should immediately
/// receive `timeout_reply` instead of hanging forever.
#[test]
fn scatter_gather_returns_timeout_reply_on_coordinator_spawn_failure() {
  let pool_size = 2_usize;
  let replies = ArcShared::new(SpinSyncMutex::new(Vec::<TestReply>::new()));
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
      let collector_ref = collector_child.into_actor_ref();

      let tr2 = tr.clone();
      let router_behavior_factory = ArcShared::new(move || {
        let mut builder = Routers::scatter_gather_first_completed_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          || responding_routee_behavior(0),
          Duration::from_millis(500),
          |msg, reply_to| match msg {
            | TestReq::Query { id, .. } => TestReq::Query { id: *id, reply_to },
          },
          |msg| match msg {
            | TestReq::Query { reply_to, .. } => Some(reply_to.clone()),
          },
          tr2.clone(),
        );
        builder.force_coord_spawn_failure = true;
        builder.build()
      });

      let rbf = router_behavior_factory.clone();
      let router_props = TypedProps::<TestReq>::from_behavior_factory(move || rbf());
      let router_child = ctx.spawn_child(&router_props).expect("router");

      Behaviors::receive_message(move |_ctx, msg: &TestReq| {
        match msg {
          | TestReq::Query { id, .. } => {
            let mut router = router_child.clone();
            router.tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
          },
        }
        Ok(Behaviors::same())
      })
    })
  });

  let ob = outer_behavior.clone();
  let props = TypedProps::<TestReq>::from_behavior_factory(move || ob());
  let system = TypedActorSystem::<TestReq>::create_from_props(
    &props,
    ActorSystemConfig::new(crate::test_support::test_tick_driver()),
  )
  .expect("system");
  let mut guardian = system.user_guardian_ref();

  let dummy_reply_to = TypedActorRef::from_untyped(ActorRef::no_sender());
  guardian.tell(TestReq::Query { id: 77, reply_to: dummy_reply_to });

  // coordinator spawn 失敗 → 即時 timeout_reply が返るはず
  wait_until(|| !replies_for_check.lock().is_empty());

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1);
  assert_eq!(got[0], timeout_reply, "coordinator spawn 失敗時は timeout_reply が即時返却されるべき");

  system.terminate().expect("terminate");
}
