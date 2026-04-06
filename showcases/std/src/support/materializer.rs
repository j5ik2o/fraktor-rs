//! Stream materializer support for std-based examples.
//!
//! Provides a `ManualTestDriver`-based materializer suitable for
//! demonstrating stream pipelines without heavyweight async runtimes.

use std::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_rs::core::{
  r#impl::StreamError,
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, StreamCompletion},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

/// Creates an `ActorMaterializer` backed by a manual test driver.
///
/// Returns the materializer (already started) and the driver handle
/// that can be used with [`drive_until_ready`] to step execution.
pub fn start_materializer() -> (ActorMaterializer, ManualTestDriver) {
  let props = Props::from_fn(|| GuardianActor);
  let driver = ManualTestDriver::new();
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(driver.clone());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  (materializer, driver)
}

/// Drives the manual test driver until the stream completes or the tick budget is exhausted.
pub fn drive_until_ready<T: Clone>(
  driver: &ManualTestDriver,
  completion: &StreamCompletion<T>,
  max_ticks: usize,
) -> Option<Result<T, StreamError>> {
  let controller = driver.controller();
  for _ in 0..max_ticks {
    controller.inject_and_drive(1);
    if let Completion::Ready(result) = completion.poll() {
      return Some(result);
    }
  }
  None
}
