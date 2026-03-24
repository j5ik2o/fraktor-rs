//! Getting started with fraktor-rs.
//!
//! Demonstrates the minimal steps to create an actor system, spawn a guardian
//! actor, and send it a message using the typed API.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example getting_started`

use fraktor_actor_rs::{
  core::typed::{Behavior, TypedActorSystem, TypedProps},
  std::typed::Behaviors,
};
use fraktor_showcases_std::support;
use fraktor_utils_rs::core::sync::SharedAccess;

// --- メッセージ定義 ---

#[derive(Clone, Copy)]
enum Command {
  Greet,
}

// --- Behavior 定義 ---

fn greeter() -> Behavior<Command> {
  Behaviors::receive_message(|_ctx, message: &Command| match message {
    | Command::Greet => {
      println!("Hello from fraktor-rs!");
      Ok(Behaviors::same())
    },
  })
}

// --- エントリーポイント ---

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // アクターシステムを起動
  let props = TypedProps::from_behavior_factory(greeter);
  let (tick_driver_config, _pulse_handle) = support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver_config).expect("system");
  let termination = system.when_terminated();

  // guardian にメッセージを送信
  let _: () = system.user_guardian_ref().tell(Command::Greet);

  // システムを終了
  // NOTE: 本番コードでは条件変数やチャネルベースの待機を使用してください
  system.terminate().expect("terminate");
  while !termination.with_read(|af| af.is_ready()) {
    thread::yield_now();
  }
}
