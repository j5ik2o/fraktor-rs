use cellactor_actor_core_rs::error::ActorError;
use cellactor_actor_std_rs::typed::{Behavior, Behaviors, TypedActorSystem, TypedProps};

#[derive(Clone, Copy)]
enum CounterCommand {
  Add(i32),
  Read,
}

fn counter(total: i32) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | CounterCommand::Add(delta) => Ok(counter(total + delta)),
    | CounterCommand::Read => {
      ctx.reply(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // 開発環境では `cargo run --example typed_behaviors_counter` で実行し、ログで結果を確認する。
  let props = TypedProps::from_behavior_factory(|| counter(0));
  let system = TypedActorSystem::new(&props).expect("system");
  let counter = system.user_guardian_ref();
  let termination = system.when_terminated();

  counter.tell(CounterCommand::Add(4)).expect("add first");
  counter.tell(CounterCommand::Add(6)).expect("add second");

  let response = counter.ask::<i32>(CounterCommand::Read).expect("ask read");
  let future = response.future().clone();
  while !future.is_ready() {
    thread::yield_now();
  }
  if let Some(result) = future.try_take() {
    match result {
      | Ok(value) => println!("typed behaviors counter result: {value}"),
      | Err(error) => println!("typed behaviors counter error: {error}"),
    }
  }

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}
