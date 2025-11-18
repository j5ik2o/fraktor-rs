#![cfg_attr(all(not(test), target_os = "none"), no_std)]

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{Behavior, Behaviors, TypedActorSystem, TypedProps},
};

#[derive(Clone, Copy)]
enum GateCommand {
  InsertCoin,
  PassThrough,
  ReadPassCount,
  Shutdown,
}

fn locked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | GateCommand::InsertCoin => {
      #[cfg(not(target_os = "none"))]
      println!("locked -> unlocked");
      Ok(unlocked(pass_count))
    },
    | GateCommand::PassThrough => {
      #[cfg(not(target_os = "none"))]
      println!("locked: PassThrough は無視されました");
      Ok(Behaviors::ignore())
    },
    | GateCommand::ReadPassCount => {
      ctx.reply(pass_count).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
    | GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}

fn unlocked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | GateCommand::PassThrough => {
      let next_total = pass_count + 1;
      #[cfg(not(target_os = "none"))]
      println!("unlocked -> locked (total {next_total})");
      Ok(locked(next_total))
    },
    | GateCommand::InsertCoin => {
      #[cfg(not(target_os = "none"))]
      println!("unlocked: 追加の InsertCoin は無視されました");
      Ok(Behaviors::ignore())
    },
    | GateCommand::ReadPassCount => {
      ctx.reply(pass_count).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
    | GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}

#[cfg(not(target_os = "none"))]
#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // `cargo run --example behaviors_state_transition_typed_no_std`
  // で実行し、出力ログで状態遷移を確認する。
  let props = TypedProps::from_behavior_factory(|| locked(0));
  let tick_driver = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let gate = system.user_guardian_ref();
  let termination = system.when_terminated();

  gate.tell(GateCommand::PassThrough).expect("fail first pass");
  gate.tell(GateCommand::InsertCoin).expect("unlock once");
  gate.tell(GateCommand::InsertCoin).expect("extra coin");
  gate.tell(GateCommand::PassThrough).expect("pass after unlock");

  let response = gate.ask::<u32>(GateCommand::ReadPassCount).expect("ask count");
  let future = response.future().clone();
  while !future.is_ready() {
    thread::yield_now();
  }
  if let Some(result) = future.try_take() {
    match result {
      | Ok(total) => println!("gate allowed {total} people"),
      | Err(error) => println!("gate count ask error: {error}"),
    }
  }

  gate.tell(GateCommand::Shutdown).expect("shutdown gate");

  system.terminate().expect("terminate system");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
