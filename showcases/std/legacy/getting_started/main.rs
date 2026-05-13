//! Getting started with fraktor-rs.
//!
//! Demonstrates the minimal steps to create an actor system, spawn a guardian
//! actor, and send it a message using the typed API.
//!
//! Run with: `cargo run -p fraktor-showcases-std --example getting_started`

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{actor::setup::ActorSystemConfig, event::logging::LogLevel};
use fraktor_actor_core_typed_rs::{Behavior, TypedActorSystem, dsl::Behaviors};
use fraktor_showcases_std::subscribe_typed_tracing_logger;

// --- メッセージ定義 ---

#[derive(Clone, Copy)]
enum Command {
  Greet,
}

// --- Behavior 定義 ---

fn greeter() -> Behavior<Command> {
  Behaviors::receive_message(|ctx, message: &Command| match message {
    | Command::Greet => {
      ctx.system().emit_log(LogLevel::Info, "Hello from fraktor-rs!", Some(ctx.pid()), None);
      Ok(Behaviors::same())
    },
  })
}

// --- エントリーポイント ---

fn main() {
  // アクターシステムを起動
  let system =
    TypedActorSystem::create_from_behavior_factory(greeter, ActorSystemConfig::new(StdTickDriver::default()))
      .expect("system");
  let _log_subscription = subscribe_typed_tracing_logger(&system);
  let termination = system.when_terminated();

  // guardian にメッセージを送信
  system.user_guardian_ref().tell(Command::Greet);
  println!("getting_started sent Greet to the guardian actor");

  // システムを終了し、TerminationSignal で安全に待機
  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}
