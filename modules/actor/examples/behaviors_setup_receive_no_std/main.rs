#![cfg_attr(all(not(test), target_os = "none"), no_std)]

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use fraktor_actor_rs::core::typed::{Behavior, Behaviors, TypedActorSystem, TypedProps};

#[derive(Clone, Copy)]
enum GuardianCommand {
  Start,
}

#[derive(Clone)]
struct WorkerCommand {
  text: &'static str,
}

fn guardian_behavior() -> Behavior<GuardianCommand> {
  Behaviors::setup(|ctx| {
    // setup 内で子アクターを生成し、後続の receiveMessage に共有する
    let worker =
      ctx.spawn_child(&TypedProps::from_behavior_factory(worker_behavior)).expect("spawn worker").actor_ref();

    Behaviors::receive_message(move |_ctx, message| match message {
      | GuardianCommand::Start => {
        worker.tell(WorkerCommand { text: "setup からの初期化メッセージ" }).expect("tell worker");
        Ok(Behaviors::same())
      },
    })
  })
}

fn worker_behavior() -> Behavior<WorkerCommand> {
  Behaviors::receive_message(|_ctx, message: &WorkerCommand| {
    #[cfg(not(target_os = "none"))]
    println!("worker: {}", message.text);
    Ok(Behaviors::same())
  })
}

#[cfg(not(target_os = "none"))]
#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // `cargo run --example behaviors_setup_receive` で実行する
  let props = TypedProps::from_behavior_factory(guardian_behavior);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(GuardianCommand::Start).expect("start");
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}

#[cfg(target_os = "none")]
fn main() {}
