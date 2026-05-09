extern crate std;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, ChildRef, Pid, error::ActorError, messaging::AnyMessageView, props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use super::DownstreamCancellationRoute;
use crate::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source},
  r#impl::{
    fusing::StreamBufferConfig,
    interpreter::IslandBoundaryShared,
    materialization::{Stream, StreamShared},
  },
  materialization::{KeepRight, StreamNotUsed},
  stage::StageKind,
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct PendingSourceLogic;

impl SourceLogic for PendingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let config = ActorSystemConfig::new(TestTickDriver::default());
  ActorSystem::create_from_props(&props, config).expect("system should build")
}

fn spawn_child_ref(system: &ActorSystem) -> ChildRef {
  let parent_pid = system.state().system_guardian_pid().expect("system guardian should exist");
  let props = Props::from_fn(|| GuardianActor);
  let mut context = ActorContext::new(system, parent_pid);
  context.spawn_child(&props).expect("child should spawn")
}

fn running_stream() -> StreamShared {
  let graph =
    Source::<u32, StreamNotUsed>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  StreamShared::new(stream)
}

impl DownstreamCancellationRoute {
  pub(in crate::materialization) fn cancel_command_count_for_actor(&self, actor_pid: Pid) -> u32 {
    if self.upstream_actor.pid() == actor_pid { self.cancel_command_count } else { 0 }
  }
}

#[test]
fn add_downstream_requires_every_downstream_to_cancel_before_reserving() {
  let system = build_system();
  let actor = spawn_child_ref(&system);
  let first_boundary = IslandBoundaryShared::new(1);
  let second_boundary = IslandBoundaryShared::new(1);
  let mut route = DownstreamCancellationRoute::new(first_boundary.clone(), running_stream(), running_stream(), actor);
  route.add_downstream(second_boundary.clone(), running_stream());

  first_boundary.cancel_downstream();
  assert!(route.reserve_cancel_target().is_none());

  second_boundary.cancel_downstream();
  let reserved = route.reserve_cancel_target().expect("all downstream watches should be cancelled");

  assert_eq!(reserved.actor_pid(), reserved.into_actor().pid());
}

#[test]
fn finish_cancel_delivery_ignores_unrelated_actor_pid() {
  let system = build_system();
  let actor = spawn_child_ref(&system);
  let actor_pid = actor.pid();
  let boundary = IslandBoundaryShared::new(1);
  boundary.cancel_downstream();
  let mut route = DownstreamCancellationRoute::new(boundary, running_stream(), running_stream(), actor);
  let reserved = route.reserve_cancel_target().expect("target should be reserved");

  assert!(!route.finish_cancel_delivery(Pid::new(99, 99), true));
  assert_eq!(route.cancel_command_count_for_actor(actor_pid), 0);
  assert!(route.finish_cancel_delivery(reserved.actor_pid(), true));
  assert_eq!(route.cancel_command_count_for_actor(actor_pid), 1);
}
