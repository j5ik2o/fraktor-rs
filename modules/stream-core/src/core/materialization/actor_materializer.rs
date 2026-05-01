use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
use core::{hint, time::Duration};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    ChildRef,
    messaging::AnyMessage,
    props::Props,
    scheduler::{ExecutionBatch, SchedulerCommand, SchedulerError, SchedulerHandle, SchedulerRunnable},
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::{
  ActorMaterializerConfig, Materialized, Materializer, MaterializerLifecycleState, RunnableGraph,
  downstream_cancellation_control_plane::DownstreamCancellationControlPlaneShared,
  downstream_cancellation_route::DownstreamCancellationRoute,
};
use crate::core::{
  KillSwitchCommandTarget, KillSwitchCommandTargetShared, KillSwitchStateHandle, KillSwitchStatus, StreamError,
  r#impl::{
    fusing::StreamBufferConfig,
    interpreter::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared, IslandSplitter, SingleIslandPlan},
    materialization::{
      Stream, StreamIslandActor, StreamIslandCommand, StreamIslandDriveGate, StreamIslandTickHandleSlot, StreamShared,
    },
  },
  materialization::empty_downstream_cancellation_control_plane,
  snapshot::MaterializerSnapshot,
  stream_ref::StreamRefSettings,
};

#[cfg(test)]
mod tests;

/// Materializer backed by an actor system.
pub struct ActorMaterializer {
  system:             Option<ActorSystem>,
  config:             ActorMaterializerConfig,
  state:              MaterializerLifecycleState,
  total_materialized: u64,
  streams:            Vec<StreamShared>,
  materialized:       Vec<MaterializedStreamResources>,
}

struct MaterializedStreamResources {
  streams: Vec<StreamShared>,
  island_actors: Vec<ChildRef>,
  drive_gates: Vec<StreamIslandDriveGate>,
  tick_handles: Vec<SchedulerHandle>,
  downstream_cancellation_control_plane: DownstreamCancellationControlPlaneShared,
}

struct DownstreamCancellationBoundary {
  upstream_island_index:   usize,
  downstream_island_index: usize,
  boundary:                IslandBoundaryShared,
}

impl DownstreamCancellationBoundary {
  const fn new(upstream_island_index: usize, downstream_island_index: usize, boundary: IslandBoundaryShared) -> Self {
    Self { upstream_island_index, downstream_island_index, boundary }
  }
}

struct GraphKillSwitchCommandTarget {
  island_actors: Vec<ChildRef>,
}

impl GraphKillSwitchCommandTarget {
  fn new(island_actors: &[ChildRef]) -> Self {
    Self { island_actors: island_actors.to_vec() }
  }

  fn send_command_to_all(&self, command: &StreamIslandCommand) -> Result<(), StreamError> {
    let mut result = Ok(());
    for actor in &self.island_actors {
      let mut actor = actor.clone();
      if let Err(error) = ActorMaterializer::send_command(&mut actor, command.clone()) {
        ActorMaterializer::record_first_error(&mut result, error);
      }
    }
    result
  }
}

impl KillSwitchCommandTarget for GraphKillSwitchCommandTarget {
  fn shutdown(&self) -> Result<(), StreamError> {
    self.send_command_to_all(&StreamIslandCommand::Shutdown)
  }

  fn abort(&self, error: StreamError) -> Result<(), StreamError> {
    self.send_command_to_all(&StreamIslandCommand::Abort(error))
  }
}

impl MaterializedStreamResources {
  const fn new(streams: Vec<StreamShared>, control_plane: DownstreamCancellationControlPlaneShared) -> Self {
    Self {
      streams,
      island_actors: Vec::new(),
      drive_gates: Vec::new(),
      tick_handles: Vec::new(),
      downstream_cancellation_control_plane: control_plane,
    }
  }
}

impl ActorMaterializer {
  /// Creates a new materializer bound to the provided actor system.
  #[must_use]
  pub const fn new(system: ActorSystem, config: ActorMaterializerConfig) -> Self {
    Self {
      system: Some(system),
      config,
      state: MaterializerLifecycleState::Idle,
      total_materialized: 0,
      streams: Vec::new(),
      materialized: Vec::new(),
    }
  }

  fn system(&self) -> Result<ActorSystem, StreamError> {
    self.system.clone().ok_or(StreamError::ActorSystemMissing)
  }

  fn send_command(actor: &mut ChildRef, command: StreamIslandCommand) -> Result<(), StreamError> {
    let message = AnyMessage::new(command);
    actor.try_tell(message).map_err(|_| StreamError::Failed)
  }

  fn send_drive_if_idle(actor: &ChildRef, drive_gate: &StreamIslandDriveGate) -> Result<(), StreamError> {
    if !drive_gate.try_mark_pending() {
      return Ok(());
    }

    let mut actor = actor.clone();
    if actor.try_tell(AnyMessage::new(StreamIslandCommand::Drive)).is_err() {
      drive_gate.mark_idle();
      return Err(StreamError::Failed);
    }
    Ok(())
  }

  fn run_scheduled_tick(actor: &ChildRef, drive_gate: &StreamIslandDriveGate) -> Result<(), StreamError> {
    Self::send_drive_if_idle(actor, drive_gate)
  }

  fn abort_streams(streams: &[StreamShared], error: &StreamError) {
    for stream in streams {
      stream.abort(error);
    }
  }

  fn schedule_ticks(
    system: &ActorSystem,
    actor: &ChildRef,
    drive_gate: StreamIslandDriveGate,
    interval: Duration,
    streams: &[StreamShared],
    tick_handle_slot: &StreamIslandTickHandleSlot,
  ) -> Result<SchedulerHandle, StreamError> {
    let actor = actor.clone();
    let streams = streams.to_vec();
    let tick_handle_slot_for_runnable = tick_handle_slot.clone();
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(move |_batch: &ExecutionBatch| {
      if let Err(error) = Self::run_scheduled_tick(&actor, &drive_gate) {
        Self::abort_streams(&streams, &error);
        if let Some(handle) = tick_handle_slot_for_runnable.lock().take() {
          let _cancelled = handle.cancel();
        }
      }
    });
    let command = SchedulerCommand::RunRunnable { runnable };
    let handle = system
      .scheduler()
      .with_write(|scheduler| {
        let handle = scheduler.schedule_at_fixed_rate(interval, interval, command)?;
        *tick_handle_slot.lock() = Some(handle.clone());
        Ok::<SchedulerHandle, SchedulerError>(handle)
      })
      .map_err(|_| StreamError::Failed)?;
    Ok(handle)
  }

  fn stream_from_island(
    island: SingleIslandPlan,
    system: &ActorSystem,
    buffer_config: StreamBufferConfig,
    kill_switch_state: KillSwitchStateHandle,
    stream_ref_settings: &StreamRefSettings,
  ) -> Result<(StreamShared, Option<String>), StreamError> {
    let dispatcher = island.dispatcher().map(String::from);
    let stream_plan = island.into_stream_plan();
    let mut stream = Stream::new_with_materializer_context(
      stream_plan,
      buffer_config,
      kill_switch_state,
      Some(system),
      stream_ref_settings,
    );
    stream.start()?;
    Ok((StreamShared::new(stream), dispatcher))
  }

  fn spawn_island_actor(
    system: &ActorSystem,
    stream: &StreamShared,
    dispatcher: Option<String>,
    drive_gate: StreamIslandDriveGate,
    downstream_cancellation_control_plane: &DownstreamCancellationControlPlaneShared,
    graph_streams: &[StreamShared],
    tick_handle_slot: &StreamIslandTickHandleSlot,
  ) -> Result<ChildRef, StreamError> {
    let actor_stream = stream.clone();
    let actor_downstream_cancellation_control_plane = downstream_cancellation_control_plane.clone();
    let actor_graph_streams = graph_streams.to_vec();
    let actor_tick_handle_slot = tick_handle_slot.clone();
    let mut props = Props::from_fn(move || {
      StreamIslandActor::new(
        actor_stream.clone(),
        drive_gate.clone(),
        actor_downstream_cancellation_control_plane.clone(),
        actor_graph_streams.clone(),
        actor_tick_handle_slot.clone(),
      )
    })
    .with_name(format!("stream-island-{}", stream.id()));
    if let Some(dispatcher_id) = dispatcher {
      props = props.with_dispatcher_id(dispatcher_id);
    }
    system.extended().spawn_system_actor(&props).map_err(|_| StreamError::Failed)
  }

  fn build_materialized_resources(
    system: &ActorSystem,
    streams: Vec<StreamShared>,
    dispatchers: Vec<Option<String>>,
    interval: Duration,
    downstream_cancellation_boundaries: Vec<DownstreamCancellationBoundary>,
    downstream_cancellation_control_plane: &DownstreamCancellationControlPlaneShared,
  ) -> Result<MaterializedStreamResources, StreamError> {
    let mut resources = MaterializedStreamResources::new(streams, downstream_cancellation_control_plane.clone());
    let actor_streams = resources.streams.clone();
    let mut tick_handle_slots = Vec::with_capacity(actor_streams.len());
    for (stream, dispatcher) in actor_streams.into_iter().zip(dispatchers) {
      let drive_gate = StreamIslandDriveGate::new();
      let tick_handle_slot = ArcShared::new(SpinSyncMutex::new(None));
      let actor = match Self::spawn_island_actor(
        system,
        &stream,
        dispatcher,
        drive_gate.clone(),
        downstream_cancellation_control_plane,
        &resources.streams,
        &tick_handle_slot,
      ) {
        | Ok(actor) => actor,
        | Err(error) => return Err(Self::rollback_materialized_resources(system, resources, error)),
      };
      resources.island_actors.push(actor);
      resources.drive_gates.push(drive_gate);
      tick_handle_slots.push(tick_handle_slot);
    }

    if let Err(error) = Self::configure_downstream_cancellation_control_plane(
      downstream_cancellation_boundaries,
      &resources.island_actors,
      &resources.streams,
      downstream_cancellation_control_plane,
    ) {
      return Err(Self::rollback_materialized_resources(system, resources, error));
    }
    let actors = resources.island_actors.clone();
    let drive_gates = resources.drive_gates.clone();
    for ((actor, drive_gate), tick_handle_slot) in actors.into_iter().zip(drive_gates).zip(tick_handle_slots) {
      let tick_handle =
        match Self::schedule_ticks(system, &actor, drive_gate, interval, &resources.streams, &tick_handle_slot) {
          | Ok(tick_handle) => tick_handle,
          | Err(error) => return Err(Self::rollback_materialized_resources(system, resources, error)),
        };
      resources.tick_handles.push(tick_handle);
    }
    Ok(resources)
  }

  fn configure_downstream_cancellation_control_plane(
    downstream_cancellation_boundaries: Vec<DownstreamCancellationBoundary>,
    island_actors: &[ChildRef],
    streams: &[StreamShared],
    downstream_cancellation_control_plane: &DownstreamCancellationControlPlaneShared,
  ) -> Result<(), StreamError> {
    let mut grouped_routes: BTreeMap<usize, DownstreamCancellationRoute> = BTreeMap::new();
    for boundary in downstream_cancellation_boundaries {
      let actor = island_actors.get(boundary.upstream_island_index).cloned().ok_or(StreamError::InvalidConnection)?;
      let upstream_stream =
        streams.get(boundary.upstream_island_index).cloned().ok_or(StreamError::InvalidConnection)?;
      let downstream_stream =
        streams.get(boundary.downstream_island_index).cloned().ok_or(StreamError::InvalidConnection)?;
      match grouped_routes.get_mut(&boundary.upstream_island_index) {
        | Some(route) => {
          route.add_downstream(boundary.boundary, downstream_stream);
        },
        | None => {
          grouped_routes.insert(
            boundary.upstream_island_index,
            DownstreamCancellationRoute::new(boundary.boundary, upstream_stream, downstream_stream, actor),
          );
        },
      }
    }
    let routes = grouped_routes.into_values().collect();
    downstream_cancellation_control_plane.lock().replace_routes(routes);
    Ok(())
  }

  fn register_graph_kill_switch_target(
    kill_switch_state: &KillSwitchStateHandle,
    island_actors: &[ChildRef],
  ) -> Result<(), StreamError> {
    let target: KillSwitchCommandTargetShared = ArcShared::new(GraphKillSwitchCommandTarget::new(island_actors));
    let status = {
      let mut state = kill_switch_state.lock();
      state.add_command_target(target.clone())
    };
    match status {
      | KillSwitchStatus::Running => Ok(()),
      | KillSwitchStatus::Shutdown => {
        Self::synchronize_graph_kill_switch_target(kill_switch_state, &target, target.shutdown())
      },
      | KillSwitchStatus::Aborted(error) => {
        Self::synchronize_graph_kill_switch_target(kill_switch_state, &target, target.abort(error))
      },
    }
  }

  fn synchronize_graph_kill_switch_target(
    kill_switch_state: &KillSwitchStateHandle,
    target: &KillSwitchCommandTargetShared,
    result: Result<(), StreamError>,
  ) -> Result<(), StreamError> {
    if let Err(error) = result {
      let removed = kill_switch_state.lock().remove_command_target(target);
      debug_assert!(removed, "registered kill switch target must be removable after synchronization failure");
      return Err(error);
    }
    Ok(())
  }

  fn register_graph_kill_switch_target_or_rollback(
    system: &ActorSystem,
    resources: MaterializedStreamResources,
    kill_switch_state: &KillSwitchStateHandle,
  ) -> Result<MaterializedStreamResources, StreamError> {
    if let Err(error) = Self::register_graph_kill_switch_target(kill_switch_state, &resources.island_actors) {
      return Err(Self::rollback_materialized_resources(system, resources, error));
    }
    Ok(resources)
  }

  fn cancel_tick(system: &ActorSystem, handle: &SchedulerHandle) -> Result<(), StreamError> {
    let cancelled = system.scheduler().with_write(|scheduler| scheduler.cancel(handle));
    if cancelled || handle.is_cancelled() || handle.is_completed() { Ok(()) } else { Err(StreamError::Failed) }
  }

  fn cancel_streams(streams: &[StreamShared]) -> Result<(), StreamError> {
    let mut result = Ok(());
    for stream in streams {
      if let Err(error) = stream.cancel() {
        Self::record_first_error(&mut result, error);
      }
    }
    result
  }

  fn request_stream_shutdown(streams: &[StreamShared]) -> Result<(), StreamError> {
    let mut result = Ok(());
    for stream in streams {
      if let Err(error) = stream.shutdown() {
        Self::record_first_error(&mut result, error);
      }
    }
    result
  }

  fn request_actor_shutdown(actors: &[ChildRef]) -> Result<(), StreamError> {
    let mut result = Ok(());
    for actor in actors {
      let mut actor = actor.clone();
      if let Err(error) = Self::send_command(&mut actor, StreamIslandCommand::Shutdown) {
        Self::record_first_error(&mut result, error);
      }
    }
    result
  }

  fn all_streams_terminal(streams: &[StreamShared]) -> bool {
    streams.iter().all(|stream| stream.state().is_terminal())
  }

  fn drive_streams_until_terminal(streams: &[StreamShared]) -> Result<(), StreamError> {
    const DIRECT_DRAIN_ROUND_LIMIT: usize = 4096;

    for _ in 0..DIRECT_DRAIN_ROUND_LIMIT {
      let mut progressed = false;
      for stream in streams {
        if stream.state().is_terminal() {
          continue;
        }
        if matches!(stream.drive(), crate::core::materialization::DriveOutcome::Progressed) {
          progressed = true;
        }
      }
      if Self::all_streams_terminal(streams) {
        return Ok(());
      }
      if !progressed {
        hint::spin_loop();
      }
    }
    Err(StreamError::failed_with_context("graceful shutdown exceeded drain round limit"))
  }

  fn drive_actor_owned_streams_until_terminal(resources: &MaterializedStreamResources) -> Result<(), StreamError> {
    if resources.island_actors.len() != resources.streams.len()
      || resources.drive_gates.len() != resources.streams.len()
    {
      return Err(StreamError::InvalidConnection);
    }

    // Shutdown has already cancelled scheduled ticks and requested stream
    // shutdown directly, so the caller owns the final bounded drain here.
    // Any already-queued actor Drive is serialized by StreamShared's mutex.
    Self::drive_streams_until_terminal(&resources.streams)
  }

  fn teardown_resources_with_command<F>(
    system: &ActorSystem,
    mut resources: MaterializedStreamResources,
    mut command_for_actor: F,
  ) -> Result<(), StreamError>
  where
    F: FnMut() -> StreamIslandCommand, {
    let mut result = Ok(());
    resources.downstream_cancellation_control_plane.lock().replace_routes(Vec::new());
    for handle in &resources.tick_handles {
      if let Err(error) = Self::cancel_tick(system, handle) {
        Self::record_first_error(&mut result, error);
      }
    }
    for actor in &mut resources.island_actors {
      if let Err(error) = Self::send_command(actor, command_for_actor()) {
        Self::record_first_error(&mut result, error);
      }
    }
    for actor in &resources.island_actors {
      if actor.stop().is_err() {
        Self::record_first_error(&mut result, StreamError::Failed);
      }
    }
    if let Err(error) = Self::cancel_streams(&resources.streams) {
      Self::record_first_error(&mut result, error);
    }
    result
  }

  fn shutdown_resources(system: &ActorSystem, resources: MaterializedStreamResources) -> Result<(), StreamError> {
    let mut result = Ok(());
    let resources = resources;
    resources.downstream_cancellation_control_plane.lock().replace_routes(Vec::new());
    for handle in &resources.tick_handles {
      if let Err(error) = Self::cancel_tick(system, handle) {
        Self::record_first_error(&mut result, error);
      }
    }
    if resources.island_actors.is_empty() {
      if let Err(error) = Self::request_stream_shutdown(&resources.streams) {
        Self::record_first_error(&mut result, error);
      }
      if let Err(error) = Self::drive_streams_until_terminal(&resources.streams) {
        Self::record_first_error(&mut result, error);
      }
    } else {
      if let Err(error) = Self::request_actor_shutdown(&resources.island_actors) {
        Self::record_first_error(&mut result, error);
      }
      if let Err(error) = Self::request_stream_shutdown(&resources.streams) {
        Self::record_first_error(&mut result, error);
      }
      if let Err(error) = Self::drive_actor_owned_streams_until_terminal(&resources) {
        Self::record_first_error(&mut result, error);
      }
    }
    for actor in &resources.island_actors {
      if actor.stop().is_err() {
        Self::record_first_error(&mut result, StreamError::Failed);
      }
    }
    if let Err(error) = Self::cancel_streams(&resources.streams) {
      Self::record_first_error(&mut result, error);
    }
    result
  }

  fn cancel_resources(
    system: &ActorSystem,
    resources: MaterializedStreamResources,
    cause: Option<&StreamError>,
  ) -> Result<(), StreamError> {
    Self::teardown_resources_with_command(system, resources, || StreamIslandCommand::Cancel { cause: cause.cloned() })
  }

  fn rollback_materialized_resources(
    system: &ActorSystem,
    resources: MaterializedStreamResources,
    primary_error: StreamError,
  ) -> StreamError {
    match Self::cancel_resources(system, resources, Some(&primary_error)) {
      | Ok(()) => primary_error,
      | Err(cleanup_error) => StreamError::materialized_resource_rollback_failed(primary_error, cleanup_error),
    }
  }

  fn record_first_error(result: &mut Result<(), StreamError>, error: StreamError) {
    if result.is_ok() {
      *result = Err(error);
    }
  }

  /// Starts the materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when startup fails.
  pub fn start(&mut self) -> Result<(), StreamError> {
    Materializer::start(self)
  }

  /// Materializes a graph into a running stream.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    Materializer::materialize(self, graph)
  }

  /// Shuts down the materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when shutdown fails.
  pub fn shutdown(&mut self) -> Result<(), StreamError> {
    Materializer::shutdown(self)
  }

  /// Returns a diagnostic snapshot of this materializer.
  #[must_use]
  pub const fn snapshot(&self) -> MaterializerSnapshot {
    MaterializerSnapshot::new(self.state, self.total_materialized)
  }

  /// Returns the current lifecycle state.
  #[must_use]
  pub const fn lifecycle_state(&self) -> MaterializerLifecycleState {
    self.state
  }

  /// Returns true if the materializer is running.
  #[must_use]
  pub const fn is_running(&self) -> bool {
    matches!(self.state, MaterializerLifecycleState::Running)
  }

  /// Returns true if the materializer has not yet been started.
  #[must_use]
  pub const fn is_idle(&self) -> bool {
    matches!(self.state, MaterializerLifecycleState::Idle)
  }

  /// Returns true if the materializer has been shut down.
  #[must_use]
  pub const fn is_stopped(&self) -> bool {
    matches!(self.state, MaterializerLifecycleState::Stopped)
  }

  /// Returns the registered streams.
  ///
  /// Used by [`crate::core::snapshot::MaterializerState::stream_snapshots`] to
  /// collect diagnostic snapshots from all active streams. The slice reflects
  /// the order in which streams were materialized, and is cleared on
  /// [`shutdown`](Self::shutdown).
  #[must_use]
  pub(in crate::core) fn streams(&self) -> &[StreamShared] {
    &self.streams
  }
}

impl Materializer for ActorMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    match self.state {
      | MaterializerLifecycleState::Running => return Err(StreamError::MaterializerAlreadyStarted),
      | MaterializerLifecycleState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerLifecycleState::Idle => {},
    }

    self.system()?;
    self.state = MaterializerLifecycleState::Running;
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    match self.state {
      | MaterializerLifecycleState::Idle => return Err(StreamError::MaterializerNotStarted),
      | MaterializerLifecycleState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerLifecycleState::Running => {},
    }
    let actor_system = self.system()?;
    let (plan, materialized) = graph.into_parts();
    let island_plan = IslandSplitter::split(plan);
    let stream_ref_settings = self.config.stream_ref_settings();
    let (mut islands, crossings) = island_plan.into_parts();
    let graph_kill_switch_state = Stream::new_running_kill_switch_state();
    let mut downstream_cancellation_boundaries = Vec::new();
    let downstream_cancellation_control_plane = empty_downstream_cancellation_control_plane();

    for crossing in crossings {
      let upstream_idx = crossing.from_island().as_usize();
      let downstream_idx = crossing.to_island().as_usize();
      let element_type = crossing.element_type();
      let boundary_capacity = islands[downstream_idx]
        .input_buffer_capacity_for_inlet(crossing.to_port())
        .unwrap_or(DEFAULT_BOUNDARY_CAPACITY);
      let boundary = IslandBoundaryShared::new(boundary_capacity);
      islands[upstream_idx].add_boundary_sink(boundary.clone(), crossing.from_port(), element_type);
      islands[downstream_idx].add_boundary_source(boundary.clone(), crossing.to_port(), element_type);
      downstream_cancellation_boundaries.push(DownstreamCancellationBoundary::new(
        upstream_idx,
        downstream_idx,
        boundary,
      ));
    }

    let mut streams = Vec::with_capacity(islands.len());
    let mut dispatchers = Vec::with_capacity(islands.len());
    for island in islands {
      match Self::stream_from_island(
        island,
        &actor_system,
        self.config.buffer_config(),
        graph_kill_switch_state.clone(),
        &stream_ref_settings,
      ) {
        | Ok((stream, dispatcher)) => {
          streams.push(stream);
          dispatchers.push(dispatcher);
        },
        | Err(error) => {
          let resources = MaterializedStreamResources::new(streams, downstream_cancellation_control_plane);
          return Err(Self::rollback_materialized_resources(&actor_system, resources, error));
        },
      }
    }

    let representative_stream = streams.first().cloned().ok_or(StreamError::Failed)?;
    let resources = Self::build_materialized_resources(
      &actor_system,
      streams,
      dispatchers,
      self.config.drive_interval(),
      downstream_cancellation_boundaries,
      &downstream_cancellation_control_plane,
    )?;
    let resources =
      Self::register_graph_kill_switch_target_or_rollback(&actor_system, resources, &graph_kill_switch_state)?;
    self.streams.extend(resources.streams.iter().cloned());
    self.materialized.push(resources);
    self.total_materialized += 1;
    Ok(Materialized::new(representative_stream, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    match self.state {
      | MaterializerLifecycleState::Idle => return Err(StreamError::MaterializerNotStarted),
      | MaterializerLifecycleState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerLifecycleState::Running => {},
    }

    // Shutdown is a one-way transition: once initiated, the materializer is
    // unconditionally Stopped regardless of whether individual teardown steps
    // succeed. Rolling back to Running after partial teardown (e.g. tick
    // cancelled but some island actors still alive) would leave a worse
    // inconsistency.
    self.state = MaterializerLifecycleState::Stopped;
    self.streams.clear();

    let system = self.system()?;
    let resources = core::mem::take(&mut self.materialized);
    let mut result = Ok(());
    for resource in resources {
      if let Err(error) = Self::shutdown_resources(&system, resource) {
        Self::record_first_error(&mut result, error);
      }
    }
    result
  }
}
