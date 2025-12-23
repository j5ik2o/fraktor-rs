use std::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, SchedulerConfig, TickDriverConfig},
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_streams_rs::core::{
  ActorMaterializerConfig, ActorMaterializerGeneric, Completion, KeepRight, Sink, Source,
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

struct GuardianActor;

impl Actor<StdToolbox> for GuardianActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    _message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn main() {
  let props = PropsGeneric::from_fn(|| GuardianActor);
  let driver = ManualTestDriver::<StdToolbox>::new();
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(driver.clone());
  let config = ActorSystemConfigGeneric::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("actor system");

  let mut materializer = ActorMaterializerGeneric::new(
    system,
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("materializer start");

  let graph = Source::single(1_u32).map(|value| value + 1).to_mat(Sink::head(), KeepRight);
  let materialized = materializer.materialize(graph).expect("materialize");
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
