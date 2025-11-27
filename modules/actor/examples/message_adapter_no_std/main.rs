#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::string::{String, ToString};

use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{
    AdapterFailure, TypedActorSystem, TypedProps,
    actor_prim::{TypedActor, TypedActorContext, TypedActorRef},
  },
};

#[derive(Clone)]
struct CounterReport {
  total: i32,
}

enum GuardianEvent {
  Start,
  AdapterReady(TypedActorRef<String>),
  Reported(CounterReport),
}

struct GuardianActor {
  adapter: Option<TypedActorRef<String>>,
  counter: Option<TypedActorRef<CounterCommand>>,
}

impl GuardianActor {
  const fn new() -> Self {
    Self { adapter: None, counter: None }
  }

  fn dispatch_samples(&self) -> Result<(), ActorError> {
    let adapter = self.adapter.as_ref().ok_or_else(|| ActorError::recoverable("adapter missing"))?;
    #[cfg(not(target_os = "none"))]
    println!("guardian: dispatching samples");
    adapter.tell("5".to_string()).map_err(|error| ActorError::from_send_error(&error))?;
    adapter.tell("3".to_string()).map_err(|error| ActorError::from_send_error(&error))?;
    adapter.tell("-2".to_string()).map_err(|error| ActorError::from_send_error(&error))?;
    Ok(())
  }
}

impl TypedActor<GuardianEvent> for GuardianActor {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, GuardianEvent>,
    message: &GuardianEvent,
  ) -> Result<(), ActorError> {
    match message {
      | GuardianEvent::Start => {
        let notify = ctx.self_ref();
        let props = TypedProps::new(move || CounterActor::new(notify.clone()));
        let counter =
          ctx.spawn_child(&props).map_err(|error| ActorError::recoverable(format!("spawn failed: {:?}", error)))?;
        self.counter = Some(counter.actor_ref());
        #[cfg(not(target_os = "none"))]
        println!("guardian: counter spawned");
      },
      | GuardianEvent::AdapterReady(adapter) => {
        #[cfg(not(target_os = "none"))]
        println!("guardian: adapter ready (counter = {})", self.counter.is_some());
        self.adapter = Some(adapter.clone());
        self.dispatch_samples()?;
        if let Some(counter) = &self.counter {
          counter
            .tell(CounterCommand::Summarize(ctx.self_ref()))
            .map_err(|error| ActorError::from_send_error(&error))?;
        }
      },
      | GuardianEvent::Reported(report) => {
        #[cfg(not(target_os = "none"))]
        println!("counter total = {}", report.total);
        ctx.system().terminate().map_err(|error| ActorError::from_send_error(&error))?;
        ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      },
    }
    Ok(())
  }
}

enum CounterCommand {
  Apply(i32),
  Summarize(TypedActorRef<GuardianEvent>),
}

struct CounterActor {
  total:  i32,
  notify: TypedActorRef<GuardianEvent>,
}

impl CounterActor {
  const fn new(notify: TypedActorRef<GuardianEvent>) -> Self {
    Self { total: 0, notify }
  }
}

impl TypedActor<CounterCommand> for CounterActor {
  fn pre_start(&mut self, ctx: &mut TypedActorContext<'_, CounterCommand>) -> Result<(), ActorError> {
    let adapter = ctx
      .message_adapter(|payload: String| {
        payload.parse::<i32>().map(CounterCommand::Apply).map_err(|_| AdapterFailure::Custom("parse error".into()))
      })
      .map_err(|error| ActorError::recoverable(format!("adapter registration failed: {:?}", error)))?;
    self.notify.tell(GuardianEvent::AdapterReady(adapter)).map_err(|error| ActorError::from_send_error(&error))?;
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, CounterCommand>,
    message: &CounterCommand,
  ) -> Result<(), ActorError> {
    match message {
      | CounterCommand::Apply(delta) => {
        #[cfg(not(target_os = "none"))]
        println!("counter: apply {delta}");
        self.total += delta;
      },
      | CounterCommand::Summarize(target) => {
        #[cfg(not(target_os = "none"))]
        println!("counter: summarize");
        let event = GuardianEvent::Reported(CounterReport { total: self.total });
        target.tell(event).map_err(|error| ActorError::from_send_error(&error))?;
        ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))?;
      },
    }
    Ok(())
  }
}

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  let props = TypedProps::new(GuardianActor::new);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("system");

  let termination = system.as_untyped().when_terminated();
  system.user_guardian_ref().tell(GuardianEvent::Start).expect("start");
  system.terminate().expect("terminate");
  while !termination.is_ready() {
    thread::yield_now();
  }
  for entry in system.as_untyped().dead_letters() {
    #[cfg(not(target_os = "none"))]
    println!("deadletter: pid={:?} reason={:?}", entry.recipient(), entry.reason());
  }
}

#[cfg(target_os = "none")]
fn main() {}
