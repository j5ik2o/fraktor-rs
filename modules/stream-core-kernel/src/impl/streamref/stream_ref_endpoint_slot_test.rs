use alloc::boxed::Box;

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{actor::scheduler::SchedulerConfig, system::ActorSystem};

use super::StreamRefEndpointSlot;
use crate::{
  StreamError,
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
};

struct NoopReceive;

impl StageActorReceive for NoopReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

#[test]
fn canonical_actor_path_requires_materialized_endpoint_actor() {
  let slot = StreamRefEndpointSlot::new();

  let error = slot.canonical_actor_path().expect_err("missing endpoint");

  assert_eq!(error, StreamError::StreamRefTargetNotInitialized);
}

#[test]
fn canonical_actor_path_uses_stage_actor_path() {
  let system = build_system();
  let stage_actor = StageActor::new(&system, Box::new(NoopReceive));
  let slot = StreamRefEndpointSlot::new();

  slot.set_actor_ref(stage_actor.actor_ref().clone());

  let canonical = slot.canonical_actor_path().expect("canonical actor path");
  assert!(canonical.starts_with("fraktor://"));
  assert!(canonical.contains("/temp/"));
}
