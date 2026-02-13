//! Basic stream example driven by an actor system.

use std::time::Duration;

use fraktor_actor_rs::{
  core::{
    error::ActorError,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
  },
  std::{
    actor::{Actor, ActorContext},
    messaging::AnyMessageView,
    props::Props,
    system::{ActorSystem, ActorSystemConfig},
  },
};
use fraktor_streams_rs::{
  core::{
    Completion, KeepRight,
    mat::ActorMaterializerConfig,
    stage::{Sink, Source},
  },
  std::ActorMaterializer,
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn main() {
  let props = Props::from_fn(|| GuardianActor);
  let driver = ManualTestDriver::new();
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(driver.clone());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver_config(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("actor system");

  let mut materializer = ActorMaterializer::new(
    system.into_core(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("materializer start");

  let graph = Source::single(1_u32).map(|value| value + 1).to_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("run");
  let controller = driver.controller();

  let mut completion = None;
  for _ in 0..5 {
    controller.inject_and_drive(1);
    match materialized.materialized().poll() {
      | Completion::Ready(result) => {
        completion = Some(result);
        break;
      },
      | Completion::Pending => {},
    }
  }
  match completion {
    | Some(Ok(value)) => println!("stream completed: {value}"),
    | Some(Err(error)) => println!("stream failed: {error:?}"),
    | None => println!("stream not completed"),
  }

  materializer.shutdown().expect("materializer shutdown");
}
