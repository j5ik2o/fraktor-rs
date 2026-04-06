//! Getting started with fraktor-rs.
//!
//! Demonstrates the minimal steps to create an actor system, spawn a guardian
//! actor, and send it a message using the typed API.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example getting_started`

use fraktor_actor_adaptor_rs::std::StdBlocker;
use fraktor_actor_rs::core::typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_showcases_std::support;

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
  // アクターシステムを起動
  let props = TypedProps::from_behavior_factory(greeter);
  let (tick_driver_config, _pulse_handle) = support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver_config).expect("system");
  let termination = system.when_terminated();

  // guardian にメッセージを送信
  system.user_guardian_ref().tell(Command::Greet);

  // システムを終了し、TerminationSignal で安全に待機
  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
