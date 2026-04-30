//! Getting started with fraktor-rs.
//!
//! Demonstrates the minimal steps to create an actor system, spawn a guardian
//! actor, and send it a message using the typed API.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example getting_started`

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{Behavior, TypedActorSystem, TypedProps, dsl::Behaviors},
};

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
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  // guardian にメッセージを送信
  system.user_guardian_ref().tell(Command::Greet);

  // システムを終了し、TerminationSignal で安全に待機
  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
