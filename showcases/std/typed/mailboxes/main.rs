use core::{num::NonZeroUsize, time::Duration};
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy)]
enum Command {
  Record(u32),
}

fn mailbox_worker(records: SharedLock<Vec<u32>>) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| {
    let Command::Record(value) = message;
    records.with_lock(|records| records.push(*value));
    Ok(Behaviors::same())
  })
}

fn main() {
  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let capacity = NonZeroUsize::new(8).expect("positive capacity");
  let props = TypedProps::from_behavior_factory({
    let records = records.clone();
    move || mailbox_worker(records.clone())
  })
  .with_mailbox_bounded(capacity);
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut actor = system.user_guardian_ref();

  for value in [1_u32, 2, 3] {
    actor.tell(Command::Record(value));
  }
  wait_until(|| records.with_lock(|records| records.len() == 3));
  let snapshot = records.with_lock(|records| records.clone());
  assert_eq!(snapshot, vec![1, 2, 3]);
  println!("typed_mailboxes delivered records: {snapshot:?}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
