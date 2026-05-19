use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_stream_core_kernel_rs::{
  StreamError,
  dsl::{Sink, Source},
  materialization::{ActorMaterializer, ActorMaterializerConfig},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_from_props(&props, config).expect("system should build")
}

fn build_materializer() -> ActorMaterializer {
  let config = ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1));
  let mut materializer = ActorMaterializer::new(build_system(), config);
  materializer.start().expect("materializer start");
  materializer
}

pub(crate) trait RunWithCollectSink<Out> {
  fn run_with_collect_sink(self) -> Result<Vec<Out>, StreamError>;
}

impl<Out, Mat> RunWithCollectSink<Out> for Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  fn run_with_collect_sink(self) -> Result<Vec<Out>, StreamError> {
    let mut materializer = build_materializer();
    let run_result = self.run_with(Sink::collect(), &mut materializer);
    let result = match run_result {
      | Ok(materialized) => {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
          if let Some(result) = materialized.materialized().try_take() {
            break result;
          }
          if Instant::now() >= deadline {
            break Err(StreamError::WouldBlock);
          }
          thread::sleep(Duration::from_millis(1));
        }
      },
      | Err(error) => Err(error),
    };
    materializer.shutdown()?;
    result
  }
}
