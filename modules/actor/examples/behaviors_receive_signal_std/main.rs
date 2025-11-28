#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use fraktor_actor_rs::std::typed::{Behavior, BehaviorSignal, Behaviors, TypedActorSystem, TypedProps};

#[derive(Clone, Copy)]
enum GuardianCommand {
  Start,
  Stop,
}

fn guardian_behavior() -> Behavior<GuardianCommand> {
  Behaviors::receive_message(|_ctx, message| match message {
    | GuardianCommand::Start => {
      #[cfg(not(target_os = "none"))]
      println!("guardian: Start を受信しました");
      Ok(Behaviors::same())
    },
    | GuardianCommand::Stop => Ok(Behaviors::stopped()),
  })
  .receive_signal(|_ctx, signal| {
    #[cfg(not(target_os = "none"))]
    match signal {
      | BehaviorSignal::Started => println!("guardian: Started signal"),
      | BehaviorSignal::Stopped => println!("guardian: Stopped signal"),
      | BehaviorSignal::Terminated(pid) => println!("guardian: Terminated({pid:?})"),
      | BehaviorSignal::AdapterFailed(reason) => println!("guardian: AdapterFailed({reason:?})"),
    }
    Ok(Behaviors::same())
  })
}

#[allow(clippy::print_stdout)]
fn main() {
  use std::thread;

  // `cargo run --example behaviors_receive_signal` で実行し、シグナルログを確認する。
  let props = TypedProps::from_behavior_factory(guardian_behavior);
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let termination = system.when_terminated();

  let guardian = system.user_guardian_ref();
  guardian.tell(GuardianCommand::Start).expect("start");
  guardian.tell(GuardianCommand::Stop).expect("stop");

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}
