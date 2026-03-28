use alloc::vec::Vec;
use core::time::Duration;

use fraktor_actor_rs::core::kernel::{
  actor::ChildRef, messaging::AnyMessage, props::Props, scheduler::SchedulerCommand, system::ActorSystem,
};
use fraktor_utils_rs::core::sync::SharedAccess;

use super::{
  ActorMaterializerConfig, Materialized, Materializer, MaterializerLifecycleState, MaterializerSnapshot, RunnableGraph,
  StreamError, StreamHandleId, StreamHandleImpl,
  lifecycle::{Stream, StreamDriveActor, StreamDriveCommand, StreamShared},
};
use crate::core::graph::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared, IslandSplitter};

#[cfg(test)]
mod tests;

/// Materializer backed by an actor system.
pub struct ActorMaterializer {
  system:             Option<ActorSystem>,
  config:             ActorMaterializerConfig,
  state:              MaterializerLifecycleState,
  drive_actor:        Option<ChildRef>,
  tick_handle:        Option<fraktor_actor_rs::core::kernel::scheduler::SchedulerHandle>,
  total_materialized: u64,
}

impl ActorMaterializer {
  /// Creates a new materializer bound to the provided actor system.
  #[must_use]
  pub const fn new(system: ActorSystem, config: ActorMaterializerConfig) -> Self {
    Self {
      system: Some(system),
      config,
      state: MaterializerLifecycleState::Idle,
      drive_actor: None,
      tick_handle: None,
      total_materialized: 0,
    }
  }

  /// Creates a materializer without an actor system (testing helper).
  #[must_use]
  pub const fn new_without_system(config: ActorMaterializerConfig) -> Self {
    Self {
      system: None,
      config,
      state: MaterializerLifecycleState::Idle,
      drive_actor: None,
      tick_handle: None,
      total_materialized: 0,
    }
  }

  fn system(&self) -> Result<ActorSystem, StreamError> {
    self.system.clone().ok_or(StreamError::ActorSystemMissing)
  }

  fn register_handle(actor: &mut ChildRef, handle: StreamHandleImpl) -> Result<(), StreamError> {
    let message = AnyMessage::new(StreamDriveCommand::Register { handle });
    actor.try_tell(message).map_err(|_| StreamError::Failed)
  }

  fn send_command(actor: &mut ChildRef, command: StreamDriveCommand) -> Result<(), StreamError> {
    let message = AnyMessage::new(command);
    actor.try_tell(message).map_err(|_| StreamError::Failed)
  }

  fn schedule_ticks(
    system: &ActorSystem,
    actor: &ChildRef,
    interval: Duration,
  ) -> Result<fraktor_actor_rs::core::kernel::scheduler::SchedulerHandle, StreamError> {
    let receiver = actor.clone().into_actor_ref();
    let command = SchedulerCommand::SendMessage {
      receiver,
      message: AnyMessage::new(StreamDriveCommand::Tick),
      dispatcher: None,
      sender: None,
    };
    system
      .scheduler()
      .with_write(|scheduler| scheduler.schedule_at_fixed_rate(interval, interval, command))
      .map_err(|_| StreamError::Failed)
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
}

impl Materializer for ActorMaterializer {
  fn start(&mut self) -> Result<(), StreamError> {
    match self.state {
      | MaterializerLifecycleState::Running => return Err(StreamError::MaterializerAlreadyStarted),
      | MaterializerLifecycleState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerLifecycleState::Idle => {},
    }

    let system = self.system()?;
    let props = Props::from_fn(StreamDriveActor::new).with_name("stream-drive");
    let drive_actor = system.extended().spawn_system_actor(&props).map_err(|_| StreamError::Failed)?;
    let interval = self.config.drive_interval();
    let tick_handle = Self::schedule_ticks(&system, &drive_actor, interval)?;

    self.drive_actor = Some(drive_actor);
    self.tick_handle = Some(tick_handle);
    self.state = MaterializerLifecycleState::Running;
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    match self.state {
      | MaterializerLifecycleState::Idle => return Err(StreamError::MaterializerNotStarted),
      | MaterializerLifecycleState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerLifecycleState::Running => {},
    }
    let drive_actor = self.drive_actor.as_mut().ok_or(StreamError::MaterializerNotStarted)?;
    let (plan, materialized) = graph.into_parts();
    let island_plan = IslandSplitter::split(plan);

    if island_plan.islands().len() <= 1 {
      // Single island: existing path
      let single_plan = island_plan.into_single_plan();
      let mut stream = Stream::new(single_plan, self.config.buffer_config());
      stream.start()?;
      let shared = StreamShared::new(stream);
      let handle = StreamHandleImpl::new(StreamHandleId::next(), shared);
      Self::register_handle(drive_actor, handle.clone())?;
      self.total_materialized += 1;
      Ok(Materialized::new(handle, materialized))
    } else {
      // Multi-island: create boundary buffers and register all streams
      let (mut islands, crossings) = island_plan.into_parts();
      for crossing in crossings {
        let upstream_idx = crossing.from_island().as_usize();
        let downstream_idx = crossing.to_island().as_usize();
        let element_type = crossing.element_type();
        let boundary_capacity = islands[downstream_idx]
          .input_buffer_capacity_for_inlet(crossing.to_port())
          .unwrap_or(DEFAULT_BOUNDARY_CAPACITY);
        let boundary = IslandBoundaryShared::new(boundary_capacity);
        islands[upstream_idx].add_boundary_sink(boundary.clone(), crossing.from_port(), element_type);
        islands[downstream_idx].add_boundary_source(boundary, crossing.to_port(), element_type);
      }
      let mut handles: Vec<StreamHandleImpl> = Vec::with_capacity(islands.len());
      for island in islands {
        let stream_plan = island.into_stream_plan();
        let mut stream = Stream::new(stream_plan, self.config.buffer_config());
        if let Err(error) = stream.start() {
          for handle in &handles {
            if let Err(_cleanup_error) = handle.cancel() {
              // Best-effort rollback: we still return the original startup failure.
            }
          }
          return Err(error);
        }
        let shared = StreamShared::new(stream);
        handles.push(StreamHandleImpl::new(StreamHandleId::next(), shared));
      }
      for handle in &handles {
        Self::register_handle(drive_actor, handle.clone())?;
      }
      let handle = handles.first().cloned().ok_or(StreamError::Failed)?;
      self.total_materialized += 1;
      Ok(Materialized::new(handle, materialized))
    }
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
    // cancelled but drive actor still alive) would leave a worse inconsistency.
    self.state = MaterializerLifecycleState::Stopped;

    let system = self.system()?;
    // cancel returns false when the job already fired or was not registered;
    // either way the tick is no longer scheduled, so this is not an error.
    if let Some(handle) = self.tick_handle.take() {
      system.scheduler().with_write(|scheduler| scheduler.cancel(&handle));
    }
    if let Some(mut actor) = self.drive_actor.take() {
      Self::send_command(&mut actor, StreamDriveCommand::Shutdown)?;
    }
    Ok(())
  }
}
