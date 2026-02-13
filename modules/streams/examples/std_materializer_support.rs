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
  core::{Completion, StreamCompletion, mat::ActorMaterializerConfig},
  std::ActorMaterializer,
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_, '_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

pub(crate) fn start_materializer() -> (ActorMaterializer, ManualTestDriver<StdToolbox>) {
  let props = Props::from_fn(|| GuardianActor);
  let driver = ManualTestDriver::<StdToolbox>::new();
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(driver.clone());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver_config(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("actor system");
  let mut materializer = ActorMaterializer::new(
    system.into_core(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("materializer start");
  (materializer, driver)
}

pub(crate) fn drive_until_ready<T: Clone>(
  driver: &ManualTestDriver<StdToolbox>,
  completion: &StreamCompletion<T>,
  max_ticks: usize,
) -> Option<Result<T, fraktor_streams_rs::core::StreamError>> {
  let controller = driver.controller();
  for _ in 0..max_ticks {
    controller.inject_and_drive(1);
    if let Completion::Ready(result) = completion.poll() {
      return Some(result);
    }
  }
  None
}
