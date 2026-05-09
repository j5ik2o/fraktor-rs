use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_core_typed_rs::{
  Behavior, TypedActorRef, TypedActorSystem, TypedProps,
  dsl::{Behaviors, routing::Routers},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone)]
enum Command {
  Work(u32),
  Read { reply_to: TypedActorRef<Vec<(usize, u32)>> },
}

fn routee(index: usize, records: SharedLock<Vec<(usize, u32)>>) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| {
    match message {
      | Command::Work(value) => records.with_lock(|records| records.push((index, *value))),
      | Command::Read { reply_to } => {
        let mut reply_to = reply_to.clone();
        reply_to.tell(records.with_lock(|records| records.clone()));
      },
    }
    Ok(Behaviors::same())
  })
}

fn router_props(records: SharedLock<Vec<(usize, u32)>>, next_index: SharedLock<usize>) -> TypedProps<Command> {
  TypedProps::from_behavior_factory(move || {
    let records = records.clone();
    let next_index = next_index.clone();
    Routers::pool::<Command, _>(2, move || {
      let index = next_index.with_lock(|next_index| {
        let current = *next_index;
        *next_index += 1;
        current
      });
      routee(index, records.clone())
    })
    .with_round_robin()
  })
}

fn main() {
  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let next_index = SharedLock::new_with_driver::<SpinSyncMutex<_>>(0_usize);
  let props = router_props(records.clone(), next_index);
  let system =
    TypedActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut router = system.user_guardian_ref();

  for value in 0..4_u32 {
    router.tell(Command::Work(value));
  }
  let snapshot = wait_for_records(&mut router);
  assert_eq!(snapshot.iter().filter(|(index, _)| *index == 0).count(), 2);
  assert_eq!(snapshot.iter().filter(|(index, _)| *index == 1).count(), 2);
  println!("typed_routers routed {} work items: {snapshot:?}", snapshot.len());

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_for_records(router: &mut TypedActorRef<Command>) -> Vec<(usize, u32)> {
  let deadline = Instant::now() + Duration::from_secs(1);
  loop {
    let response = router.ask::<Vec<(usize, u32)>, _>(|reply_to| Command::Read { reply_to });
    let mut future = response.future().clone();
    while !future.is_ready() {
      assert!(Instant::now() < deadline, "router read should complete");
      thread::sleep(Duration::from_millis(1));
    }
    let records = future.try_take().expect("ready").expect("ok");
    if records.len() == 4 {
      return records;
    }
    assert!(Instant::now() < deadline, "all routees should receive the work");
    thread::sleep(Duration::from_millis(1));
  }
}
