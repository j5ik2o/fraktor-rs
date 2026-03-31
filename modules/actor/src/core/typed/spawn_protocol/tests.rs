use core::sync::atomic::{AtomicUsize, Ordering};
use std::{
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use crate::core::typed::{SpawnProtocol, TypedActorRef, dsl::Behaviors, props::TypedProps, system::TypedActorSystem};

#[derive(Clone)]
enum ProbeCommand {
  Ping,
}

#[derive(Clone)]
enum OtherProbeCommand {
  Pong,
}

fn probe_props(start_count: &Arc<AtomicUsize>) -> TypedProps<ProbeCommand> {
  let start_count = Arc::clone(start_count);
  TypedProps::from_behavior_factory(move || {
    let start_count = Arc::clone(&start_count);
    Behaviors::receive_message(move |_ctx, _message: &ProbeCommand| Ok(Behaviors::same())).receive_signal(
      move |_ctx, signal| {
        if matches!(signal, crate::core::typed::message_and_signals::BehaviorSignal::Started) {
          start_count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(Behaviors::same())
      },
    )
  })
}

fn other_probe_props(start_count: &Arc<AtomicUsize>) -> TypedProps<OtherProbeCommand> {
  let start_count = Arc::clone(start_count);
  TypedProps::from_behavior_factory(move || {
    let start_count = Arc::clone(&start_count);
    Behaviors::receive_message(move |_ctx, _message: &OtherProbeCommand| Ok(Behaviors::same())).receive_signal(
      move |_ctx, signal| {
        if matches!(signal, crate::core::typed::message_and_signals::BehaviorSignal::Started) {
          start_count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(Behaviors::same())
      },
    )
  })
}

fn wait_until(predicate: impl Fn() -> bool) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if predicate() {
      return;
    }
    thread::yield_now();
  }
  panic!("condition not satisfied within timeout");
}

#[test]
fn spawn_protocol_spawns_named_children() {
  let start_count = Arc::new(AtomicUsize::new(0));
  let props = TypedProps::<SpawnProtocol>::from_behavior_factory(SpawnProtocol::behavior);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<SpawnProtocol>::new(&props, tick_driver).expect("system");
  let mut parent = system.user_guardian_ref();

  let response = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(probe_props(&start_count), "child", reply_to)
  });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let child = future.try_take().expect("reply").expect("child ref");

  assert!(child.pid().value() > 0);
  wait_until(|| start_count.load(Ordering::SeqCst) == 1);

  system.terminate().expect("terminate");
}

#[test]
fn spawn_protocol_spawns_anonymous_children() {
  let start_count = Arc::new(AtomicUsize::new(0));
  let props = TypedProps::<SpawnProtocol>::from_behavior_factory(SpawnProtocol::behavior);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<SpawnProtocol>::new(&props, tick_driver).expect("system");
  let mut parent = system.user_guardian_ref();

  let response = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn_anonymous(probe_props(&start_count), reply_to)
  });
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  let child = future.try_take().expect("reply").expect("child ref");

  child.clone().tell(ProbeCommand::Ping);
  wait_until(|| start_count.load(Ordering::SeqCst) == 1);

  system.terminate().expect("terminate");
}

#[test]
fn spawn_protocol_spawns_children_with_different_message_types() {
  let first_start_count = Arc::new(AtomicUsize::new(0));
  let second_start_count = Arc::new(AtomicUsize::new(0));
  let props = TypedProps::<SpawnProtocol>::from_behavior_factory(SpawnProtocol::behavior);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<SpawnProtocol>::new(&props, tick_driver).expect("system");
  let mut parent = system.user_guardian_ref();

  let first = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(probe_props(&first_start_count), "first", reply_to)
  });
  let second = parent.ask::<TypedActorRef<OtherProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(other_probe_props(&second_start_count), "second", reply_to)
  });

  let mut first_future = first.future().clone();
  let mut second_future = second.future().clone();
  wait_until(|| first_future.is_ready() && second_future.is_ready());
  let mut first_ref = first_future.try_take().expect("first reply").expect("first child");
  let mut second_ref = second_future.try_take().expect("second reply").expect("second child");

  first_ref.tell(ProbeCommand::Ping);
  second_ref.tell(OtherProbeCommand::Pong);
  wait_until(|| first_start_count.load(Ordering::SeqCst) == 1 && second_start_count.load(Ordering::SeqCst) == 1);

  system.terminate().expect("terminate");
}

#[test]
fn spawn_protocol_survives_duplicate_named_spawn_failure() {
  let start_count = Arc::new(AtomicUsize::new(0));
  let props = TypedProps::<SpawnProtocol>::from_behavior_factory(SpawnProtocol::behavior);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<SpawnProtocol>::new(&props, tick_driver).expect("system");
  let mut parent = system.user_guardian_ref();

  // First spawn with name "probe" succeeds.
  let first = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(probe_props(&start_count), "probe", reply_to)
  });
  let mut first_future = first.future().clone();
  wait_until(|| first_future.is_ready());
  first_future.try_take().expect("first reply").expect("first child ref");
  wait_until(|| start_count.load(Ordering::SeqCst) == 1);

  // Duplicate name spawn: execute() fails internally; the actor must stay alive.
  let dup = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(probe_props(&Arc::new(AtomicUsize::new(0))), "probe", reply_to)
  });
  let dup_future = dup.future().clone();

  // Prove the SpawnProtocol actor is still alive by spawning a survivor.
  // Because the mailbox is FIFO, when the survivor reply arrives the duplicate
  // command has already been processed — so we can assert no reply was sent.
  let survivor_count = Arc::new(AtomicUsize::new(0));
  let survivor = parent.ask::<TypedActorRef<ProbeCommand>, _>(|reply_to| {
    SpawnProtocol::spawn(probe_props(&survivor_count), "survivor", reply_to)
  });
  let mut survivor_future = survivor.future().clone();
  wait_until(|| survivor_future.is_ready());
  let survivor_ref = survivor_future.try_take().expect("survivor reply").expect("survivor child ref");

  assert!(!dup_future.is_ready(), "duplicate spawn must not receive a reply");
  assert!(survivor_ref.pid().value() > 0);
  wait_until(|| survivor_count.load(Ordering::SeqCst) == 1);

  system.terminate().expect("terminate");
}
