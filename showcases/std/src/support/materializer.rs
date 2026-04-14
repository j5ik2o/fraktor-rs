//! Stream materializer support for std-based examples.
//!
//! Provides a `StdTickDriver`-based materializer suitable for
//! demonstrating stream pipelines.

use std::time::Duration;

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, setup::ActorSystemConfig},
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

/// Creates an `ActorMaterializer` backed by `StdTickDriver`.
///
/// Returns the materializer (already started).
pub fn start_materializer() -> ActorMaterializer {
  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(StdTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("actor system");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("materializer start");
  materializer
}

/// Polls the stream completion until it resolves or the iteration budget is exhausted.
///
/// Each iteration sleeps for 1 ms to yield time to the `StdTickDriver` background thread.
pub fn drive_until_ready<T: Clone>(
  completion: &StreamCompletion<T>,
  max_ticks: usize,
) -> Option<Result<T, StreamError>> {
  for _ in 0..max_ticks {
    if let Completion::Ready(result) = completion.poll() {
      return Some(result);
    }
    std::thread::sleep(Duration::from_millis(1));
  }
  None
}
