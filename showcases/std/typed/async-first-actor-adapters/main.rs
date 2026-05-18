use core::time::Duration;
use std::time::Instant;

use fraktor_actor_adaptor_std_rs::actor::tokio_actor_system_config;
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::DEFAULT_BLOCKING_DISPATCHER_ID;
use fraktor_actor_core_typed_rs::{Behavior, TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};
use tokio::{runtime::Handle, time::sleep};

#[derive(Clone, Debug, PartialEq, Eq)]
struct AsyncSnapshot {
  events: Vec<&'static str>,
  value:  Option<u32>,
}

#[derive(Clone)]
enum AsyncCommand {
  Start,
  Completed(u32),
  Failed,
  Read { reply_to: TypedActorRef<AsyncSnapshot> },
}

#[derive(Clone, Copy)]
enum BlockingCommand {
  Run,
}

fn async_worker(events: Vec<&'static str>, value: Option<u32>) -> Behavior<AsyncCommand> {
  Behaviors::receive_message(move |ctx, message: &AsyncCommand| match message {
    | AsyncCommand::Start => {
      let mut next_events = events.clone();
      next_events.push("async-started");
      ctx
        .pipe_to_self(
          async {
            sleep(Duration::from_millis(10)).await;
            Ok::<u32, &'static str>(41)
          },
          |value| Ok(AsyncCommand::Completed(value + 1)),
          |_error| Ok(AsyncCommand::Failed),
        )
        .expect("pipe to self");
      Ok(async_worker(next_events, value))
    },
    | AsyncCommand::Completed(completed) => {
      let mut next_events = events.clone();
      next_events.push("pipe-to-self-completed");
      Ok(async_worker(next_events, Some(*completed)))
    },
    | AsyncCommand::Failed => {
      let mut next_events = events.clone();
      next_events.push("pipe-to-self-failed");
      Ok(async_worker(next_events, None))
    },
    | AsyncCommand::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(AsyncSnapshot { events: events.clone(), value });
      Ok(Behaviors::same())
    },
  })
}

fn blocking_worker(events: SharedLock<Vec<&'static str>>) -> Behavior<BlockingCommand> {
  Behaviors::receive_message(move |_ctx, message: &BlockingCommand| {
    if matches!(message, BlockingCommand::Run) {
      events.with_lock(|events| events.push("blocking-dispatcher-work"));
    }
    Ok(Behaviors::same())
  })
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
  let system = TypedActorSystem::create_from_behavior_factory(
    || async_worker(Vec::new(), None),
    tokio_actor_system_config(Handle::current()),
  )
  .expect("system");
  let termination = system.when_terminated();
  let mut async_actor = system.user_guardian_ref();

  let blocking_events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let blocking_props = TypedProps::from_behavior_factory({
    let blocking_events = blocking_events.clone();
    move || blocking_worker(blocking_events.clone())
  })
  .with_dispatcher_from_config(DEFAULT_BLOCKING_DISPATCHER_ID);
  let mut blocking_actor =
    system.system_actor_of(&blocking_props, "async-first-blocking-worker").expect("blocking actor");

  async_actor.tell(AsyncCommand::Start);
  blocking_actor.tell(BlockingCommand::Run);

  let snapshot = wait_for_completion(&mut async_actor, &blocking_events).await;
  assert_eq!(snapshot.events, vec!["async-started", "pipe-to-self-completed"]);
  assert_eq!(snapshot.value, Some(42));
  assert_eq!(blocking_events.with_lock(|events| events.clone()), vec!["blocking-dispatcher-work"]);
  println!("typed_async_first_actor_adapters snapshot: {snapshot:?}");

  system.terminate().expect("terminate");
  termination.await;
}

async fn wait_for_completion(
  actor: &mut TypedActorRef<AsyncCommand>,
  blocking_events: &SharedLock<Vec<&'static str>>,
) -> AsyncSnapshot {
  let deadline = Instant::now() + Duration::from_secs(2);
  loop {
    let response = actor.ask::<AsyncSnapshot, _>(|reply_to| AsyncCommand::Read { reply_to });
    let mut future = response.future().clone();
    while !future.is_ready() {
      assert!(Instant::now() < deadline, "async-first actor adapters example timed out");
      sleep(Duration::from_millis(1)).await;
    }
    let snapshot = future.try_take().expect("snapshot ready").expect("snapshot ok");
    let blocking_done = blocking_events.with_lock(|events| events.as_slice() == ["blocking-dispatcher-work"]);
    if snapshot.value == Some(42) && blocking_done {
      return snapshot;
    }
    assert!(Instant::now() < deadline, "async-first actor adapters example timed out");
    sleep(Duration::from_millis(1)).await;
  }
}
