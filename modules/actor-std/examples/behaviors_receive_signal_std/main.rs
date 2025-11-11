use fraktor_actor_std_rs::typed::{Behavior, BehaviorSignal, Behaviors, TypedActorSystem, TypedProps};

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
  let system = TypedActorSystem::new(&props).expect("system");
  let termination = system.when_terminated();

  let guardian = system.user_guardian_ref();
  guardian.tell(GuardianCommand::Start).expect("start");
  guardian.tell(GuardianCommand::Stop).expect("stop");

  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
}
