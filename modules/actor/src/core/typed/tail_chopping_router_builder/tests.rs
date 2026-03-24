extern crate std;

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
    std::thread::yield_now();
  }
  assert!(condition(), "wait_until timed out");
}

fn responding_routee_behavior(source: usize) -> Behavior<TestReq> {
  Behaviors::receive_message(move |_ctx, msg: &TestReq| {
    match msg {
      | TestReq::Query { id, reply_to } => {
        let _: () = reply_to.clone().tell(TestReply { id: *id, source });
      },
    }
    Ok(Behaviors::same())
  })
}

fn silent_routee_behavior() -> Behavior<TestReq> {
  Behaviors::receive_message(|_ctx, _msg: &TestReq| Ok(Behaviors::same()))
}

#[test]
fn tail_chopping_builder_builds_behavior() {
  let _behavior: Behavior<TestReq> = Routers::tail_chopping_pool::<TestReq, TestReply, _, _, _>(
    3,
    || responding_routee_behavior(0),
    Duration::from_secs(5),
    Duration::from_millis(100),
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
fn tail_chopping_builder_rejects_zero_pool_size() {
  let _builder = Routers::tail_chopping_pool::<TestReq, TestReply, _, _, _>(
    0,
    || responding_routee_behavior(0),
    Duration::from_secs(5),
    Duration::from_millis(100),
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
fn tail_chopping_returns_first_reply() {
  let pool_size = 3_usize;
  let next_source = ArcShared::new(NoStdMutex::new(0_usize));
  let replies = ArcShared::new(NoStdMutex::new(Vec::<TestReply>::new()));
  let replies_for_check = replies.clone();

  let ns = next_source.clone();
  let routee_factory = ArcShared::new(move || -> Behavior<TestReq> {
    let source = {
      let mut guard = ns.lock();
      let s = *guard;
      *guard += 1;
      s
    };
    responding_routee_behavior(source)
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
        Routers::tail_chopping_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          {
            let rf3 = rf2.clone();
            move || rf3()
          },
          Duration::from_secs(5),
          Duration::from_millis(50),
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
            let _: () = router_ref.clone().tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
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
  let _: () = guardian.tell(TestReq::Query { id: 77, reply_to: dummy_reply_to });

  wait_until(|| !replies_for_check.lock().is_empty());

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1, "tail-chopping should return exactly one reply");
  assert_eq!(got[0].id, 77);
  assert!(got[0].source < pool_size, "source should be a valid routee index");

  system.terminate().expect("terminate");
}

/// tail-chopping 固有の振る舞いを検証: 1台目が無応答でも
/// interval 経過後に 2台目以降に送信されて応答が得られることを確認する。
/// scatter-gather 的な実装（全台同時送信）ではこのテストは通らない。
#[test]
fn tail_chopping_retries_to_next_routee_after_interval() {
  let pool_size = 2_usize;
  let replies = ArcShared::new(NoStdMutex::new(Vec::<TestReply>::new()));
  let replies_for_check = replies.clone();
  // routee 0 は silent（無応答）、routee 1 は responsive
  let routee_index = ArcShared::new(NoStdMutex::new(0_usize));

  let ri = routee_index.clone();
  let routee_factory = ArcShared::new(move || -> Behavior<TestReq> {
    let idx = {
      let mut guard = ri.lock();
      let i = *guard;
      *guard += 1;
      i
    };
    if idx == 0 {
      // 1台目: 無応答
      silent_routee_behavior()
    } else {
      // 2台目以降: 応答する
      responding_routee_behavior(idx)
    }
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
        Routers::tail_chopping_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          {
            let rf3 = rf2.clone();
            move || rf3()
          },
          Duration::from_secs(5),
          Duration::from_millis(20),
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
            let _: () = router_ref.clone().tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
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
  let _: () = guardian.tell(TestReq::Query { id: 99, reply_to: dummy_reply_to });

  // メッセージ伝播後にスケジューラを駆動し interval タイマーを発火させる
  for _ in 0..20 {
    spin_loop();
  }
  controller.inject_and_drive(100);

  wait_until(|| !replies_for_check.lock().is_empty());

  let got = replies_for_check.lock();
  assert_eq!(got.len(), 1, "tail-chopping は1件の応答を返すべき");
  assert_eq!(got[0].id, 99);
  // 応答は routee 1 (responsive) から来るべき（routee 0 は silent）
  assert_eq!(got[0].source, 1, "1台目が無応答のため 2台目(index=1)から応答されるべき");

  system.terminate().expect("terminate");
}

#[test]
fn tail_chopping_returns_timeout_reply_when_no_routee_responds() {
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
        Routers::tail_chopping_pool::<TestReq, TestReply, _, _, _>(
          pool_size,
          silent_routee_behavior,
          Duration::from_millis(10),
          Duration::from_millis(3),
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
            let _: () = router_ref.clone().tell(TestReq::Query { id: *id, reply_to: collector_ref.clone() });
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
  let _: () = guardian.tell(TestReq::Query { id: 88, reply_to: dummy_reply_to });

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
