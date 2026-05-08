extern crate std;

use std::{
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  super::downstream_cancellation_route::DownstreamCancellationRoute, DownstreamCancellationControlPlane,
  DownstreamCancellationControlPlaneShared,
};
use crate::core::{
  DynValue, SourceLogic, StreamError,
  dsl::{Sink, Source},
  r#impl::{
    fusing::StreamBufferConfig,
    interpreter::IslandBoundaryShared,
    materialization::{
      Stream, StreamIslandActor, StreamIslandCommand, StreamIslandDriveGate, StreamShared, StreamState,
    },
  },
  materialization::{KeepRight, StreamNotUsed},
  stage::StageKind,
};

impl DownstreamCancellationControlPlane {
  pub(in crate::core::materialization) fn cancel_command_count_for_actor(&self, actor_pid: Pid) -> u32 {
    self.routes.iter().map(|route| route.cancel_command_count_for_actor(actor_pid)).sum()
  }
}

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

fn wait_for_actor_cell_removed(system: &ActorSystem, pid: Pid) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if system.state().cell(&pid).is_none() {
      return;
    }
    thread::yield_now();
  }
  assert!(system.state().cell(&pid).is_none());
}

#[test]
fn replace_routes_keeps_empty_control_plane_healthy() {
  let mut control_plane = DownstreamCancellationControlPlane::new(Vec::new());

  control_plane.replace_routes(Vec::new());
  let result = control_plane.reserve_cancellation_targets();

  assert!(result.is_empty());
}

#[test]
fn reserve_targets_blocks_duplicate_delivery_until_completion_is_recorded() {
  let system = build_system();
  let actor = spawn_child_ref(&system);
  let boundary = IslandBoundaryShared::new(1);
  boundary.cancel_downstream();
  let upstream_stream = running_stream();
  let downstream_stream = running_stream();
  let route = DownstreamCancellationRoute::new(boundary, upstream_stream, downstream_stream, actor.clone());
  let mut control_plane = DownstreamCancellationControlPlane::new(vec![route]);

  let reserved = control_plane.reserve_cancellation_targets();
  assert_eq!(reserved.len(), 1);
  let actor_pid = reserved[0].actor_pid();

  assert!(control_plane.reserve_cancellation_targets().is_empty());

  control_plane.finish_cancellation_delivery(actor_pid, false);

  assert_eq!(control_plane.reserve_cancellation_targets().len(), 1);
}

#[test]
fn successful_delivery_records_cancel_count_without_requiring_lock_held_during_send() {
  let system = build_system();
  let actor = spawn_child_ref(&system);
  let actor_pid = actor.pid();
  let boundary = IslandBoundaryShared::new(1);
  boundary.cancel_downstream();
  let upstream_stream = running_stream();
  let downstream_stream = running_stream();
  let route = DownstreamCancellationRoute::new(boundary, upstream_stream, downstream_stream, actor);
  let mut control_plane = DownstreamCancellationControlPlane::new(vec![route]);

  let reserved = control_plane.reserve_cancellation_targets();
  assert_eq!(reserved.len(), 1);

  let mut reentrant = control_plane.reserve_cancellation_targets();
  assert!(reentrant.is_empty());

  control_plane.finish_cancellation_delivery(actor_pid, true);

  reentrant = control_plane.reserve_cancellation_targets();
  assert!(reentrant.is_empty());
  assert_eq!(control_plane.cancel_command_count_for_actor(actor_pid), 1);
}

#[test]
fn finish_cancellation_delivery_ignores_unrelated_actor_pid() {
  let system = build_system();
  let actor = spawn_child_ref(&system);
  let actor_pid = actor.pid();
  let boundary = IslandBoundaryShared::new(1);
  boundary.cancel_downstream();
  let route = DownstreamCancellationRoute::new(boundary, running_stream(), running_stream(), actor);
  let mut control_plane = DownstreamCancellationControlPlane::new(vec![route]);

  let reserved = control_plane.reserve_cancellation_targets();
  assert_eq!(reserved.len(), 1);

  control_plane.finish_cancellation_delivery(Pid::new(99, 99), true);

  assert!(control_plane.reserve_cancellation_targets().is_empty());
  assert_eq!(control_plane.cancel_command_count_for_actor(actor_pid), 0);
  control_plane.finish_cancellation_delivery(reserved[0].actor_pid(), false);
  assert_eq!(control_plane.reserve_cancellation_targets().len(), 1);
}

#[test]
fn failed_delivery_aborts_graph_streams_and_finishes_in_flight_reservation() {
  let system = build_system();
  let upstream_actor = spawn_child_ref(&system);
  let upstream_actor_pid = upstream_actor.pid();
  upstream_actor.stop().expect("upstream actor should stop");
  wait_for_actor_cell_removed(&system, upstream_actor_pid);
  let boundary = IslandBoundaryShared::new(1);
  boundary.cancel_downstream();
  let upstream_stream = running_stream();
  let owned_stream = running_stream();
  let route = DownstreamCancellationRoute::new(boundary, upstream_stream.clone(), running_stream(), upstream_actor);
  let control_plane =
    DownstreamCancellationControlPlaneShared::new(DownstreamCancellationControlPlane::new(vec![route]));
  // The propagator's fast path skips the lock when this latch is false; we
  // pre-arm it so the test does not have to also wire up a boundary signal.
  control_plane.arm_pending();
  let tick_handle_slot = ArcShared::new(SpinSyncMutex::new(None));
  let mut island_actor = StreamIslandActor::new(
    owned_stream.clone(),
    StreamIslandDriveGate::new(),
    control_plane.clone(),
    vec![upstream_stream.clone(), owned_stream.clone()],
    tick_handle_slot,
  );
  let parent_pid = system.state().system_guardian_pid().expect("system guardian should exist");
  let mut context = ActorContext::new(&system, parent_pid);
  let message = AnyMessage::new(StreamIslandCommand::Drive);

  let result = island_actor.receive(&mut context, message.as_view());

  assert!(result.is_err());
  assert_eq!(upstream_stream.state(), StreamState::Failed);
  assert_eq!(owned_stream.state(), StreamState::Failed);
  assert!(control_plane.with_locked(|plane| plane.reserve_cancellation_targets()).is_empty());
}
