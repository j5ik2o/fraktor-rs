use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{Behavior, TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors},
};

#[derive(Clone)]
enum Command {
  Coin,
  Pass,
  Read { reply_to: TypedActorRef<u32> },
}

fn locked(pass_count: u32) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| match message {
    | Command::Coin => Ok(open(pass_count)),
    | Command::Pass => Ok(Behaviors::same()),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(pass_count);
      Ok(Behaviors::same())
    },
  })
}

fn open(pass_count: u32) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message: &Command| match message {
    | Command::Pass => Ok(locked(pass_count + 1)),
    | Command::Coin => Ok(Behaviors::same()),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(pass_count);
      Ok(Behaviors::same())
    },
  })
}

fn main() {
  let props = TypedProps::from_behavior_factory(|| locked(0));
  let system =
    TypedActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();
  let mut gate = system.user_guardian_ref();

  gate.tell(Command::Pass);
  gate.tell(Command::Coin);
  gate.tell(Command::Pass);
  let pass_count = read_pass_count(&mut gate);
  assert_eq!(pass_count, 1);
  println!("typed_fsm recorded pass count: {pass_count}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn read_pass_count(gate: &mut TypedActorRef<Command>) -> u32 {
  let response = gate.ask::<u32, _>(|reply_to| Command::Read { reply_to });
  let mut future = response.future().clone();
  let deadline = Instant::now() + Duration::from_secs(1);
  while !future.is_ready() {
    assert!(Instant::now() < deadline, "typed FSM read should complete");
    thread::sleep(Duration::from_millis(1));
  }
  future.try_take().expect("ready").expect("ok")
}
