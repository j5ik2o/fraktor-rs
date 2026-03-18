//! Demonstrates `Behaviors::log_messages_with_opts` with std tracing output.

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use core::fmt;
use std::{thread, time::Duration};

use fraktor_actor_rs::{
  core::{
    error::ActorError,
    typed::{Behavior, TypedActorSystem, TypedProps, actor::TypedActorRef},
  },
  std::typed::{Behaviors, LogOptions},
};
use fraktor_utils_rs::core::sync::SharedAccess;

#[derive(Clone)]
enum Command {
  Ping,
  Read { reply_to: TypedActorRef<usize> },
}

impl fmt::Debug for Command {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Ping => f.write_str("Ping"),
      | Self::Read { .. } => f.write_str("Read { reply_to: _ }"),
    }
  }
}

fn counter(total: usize) -> Behavior<Command> {
  Behaviors::log_messages_with_opts(
    LogOptions::new().with_level(tracing::Level::INFO).with_logger_name("typed.behaviors.example"),
    counter_inner(total),
  )
}

fn counter_inner(total: usize) -> Behavior<Command> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | Command::Ping => Ok(counter_inner(total + 1)),
    | Command::Read { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(total).map_err(|error| ActorError::from_send_error(&error))?;
      Ok(Behaviors::same())
    },
  })
}

#[allow(clippy::print_stdout)]
fn main() {
  let subscriber = tracing_subscriber::FmtSubscriber::builder().with_max_level(tracing::Level::INFO).finish();
  tracing::subscriber::set_global_default(subscriber).expect("subscriber");

  let props = TypedProps::from_behavior_factory(|| counter(0));
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(Command::Ping).expect("first ping");
  actor.tell(Command::Ping).expect("second ping");

  let response = actor.ask::<usize, _>(|reply_to| Command::Read { reply_to }).expect("ask");
  let mut future = response.future().clone();
  while !future.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
  let value = future.try_take().expect("ready").expect("ok");
  println!("logged ping count = {value}");

  system.terminate().expect("terminate");
  let termination = system.when_terminated();
  while !termination.with_read(|inner| inner.is_ready()) {
    thread::sleep(Duration::from_millis(10));
  }
}
