use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::ChildRefGeneric, messaging::AnyMessageGeneric, props::PropsGeneric, scheduler::SchedulerCommand,
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use super::{
  ActorMaterializerConfig, Materialized, Materializer, RunnableGraph, StreamError, StreamHandleGeneric, StreamHandleId,
  stream::Stream, stream_drive_actor::StreamDriveActor, stream_drive_command::StreamDriveCommand,
  stream_shared::StreamSharedGeneric,
};

#[cfg(test)]
mod tests;

/// Materializer backed by an actor system.
pub struct ActorMaterializerGeneric<TB: RuntimeToolbox + 'static> {
  system:      Option<ActorSystemGeneric<TB>>,
  config:      ActorMaterializerConfig,
  state:       MaterializerState,
  drive_actor: Option<ChildRefGeneric<TB>>,
  tick_handle: Option<fraktor_actor_rs::core::scheduler::SchedulerHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializerState {
  Idle,
  Running,
  Stopped,
}

impl<TB: RuntimeToolbox + 'static> ActorMaterializerGeneric<TB> {
  /// Creates a new materializer bound to the provided actor system.
  #[must_use]
  pub const fn new(system: ActorSystemGeneric<TB>, config: ActorMaterializerConfig) -> Self {
    Self { system: Some(system), config, state: MaterializerState::Idle, drive_actor: None, tick_handle: None }
  }

  /// Creates a materializer without an actor system (testing helper).
  #[must_use]
  pub const fn new_without_system(config: ActorMaterializerConfig) -> Self {
    Self { system: None, config, state: MaterializerState::Idle, drive_actor: None, tick_handle: None }
  }

  fn system(&self) -> Result<ActorSystemGeneric<TB>, StreamError> {
    self.system.clone().ok_or(StreamError::ActorSystemMissing)
  }

  fn register_handle(actor: &ChildRefGeneric<TB>, handle: StreamHandleGeneric<TB>) -> Result<(), StreamError> {
    let message = AnyMessageGeneric::new(StreamDriveCommand::Register { handle });
    actor.actor_ref().tell(message).map_err(|_| StreamError::Failed)
  }

  fn send_command(actor: &ChildRefGeneric<TB>, command: StreamDriveCommand<TB>) -> Result<(), StreamError> {
    let message = AnyMessageGeneric::new(command);
    actor.actor_ref().tell(message).map_err(|_| StreamError::Failed)
  }

  fn schedule_ticks(
    system: &ActorSystemGeneric<TB>,
    actor: &ChildRefGeneric<TB>,
    interval: Duration,
  ) -> Result<fraktor_actor_rs::core::scheduler::SchedulerHandle, StreamError> {
    let receiver = actor.actor_ref().clone();
    let command = SchedulerCommand::SendMessage {
      receiver,
      message: AnyMessageGeneric::new(StreamDriveCommand::<TB>::Tick),
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
  pub fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat, TB>, StreamError> {
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
}

impl<TB: RuntimeToolbox + 'static> Materializer for ActorMaterializerGeneric<TB> {
  type Toolbox = TB;

  fn start(&mut self) -> Result<(), StreamError> {
    match self.state {
      | MaterializerState::Running => return Err(StreamError::MaterializerAlreadyStarted),
      | MaterializerState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerState::Idle => {},
    }

    let system = self.system()?;
    let props = PropsGeneric::from_fn(StreamDriveActor::<TB>::new).with_name("stream-drive");
    let drive_actor = system.extended().spawn_system_actor(&props).map_err(|_| StreamError::Failed)?;
    let interval = self.config.drive_interval();
    let tick_handle = Self::schedule_ticks(&system, &drive_actor, interval)?;

    self.drive_actor = Some(drive_actor);
    self.tick_handle = Some(tick_handle);
    self.state = MaterializerState::Running;
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat, TB>, StreamError> {
    match self.state {
      | MaterializerState::Idle => return Err(StreamError::MaterializerNotStarted),
      | MaterializerState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerState::Running => {},
    }
    let drive_actor = self.drive_actor.as_ref().ok_or(StreamError::MaterializerNotStarted)?;
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, self.config.buffer_config());
    stream.start()?;
    let shared = StreamSharedGeneric::new(stream);
    let handle = StreamHandleGeneric::new(StreamHandleId::next(), shared);
    Self::register_handle(drive_actor, handle.clone())?;
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    match self.state {
      | MaterializerState::Idle => return Err(StreamError::MaterializerNotStarted),
      | MaterializerState::Stopped => return Err(StreamError::MaterializerStopped),
      | MaterializerState::Running => {},
    }

    self.state = MaterializerState::Stopped;
    let system = match self.system() {
      | Ok(system) => system,
      | Err(error) => return Err(error),
    };
    if let Some(handle) = self.tick_handle.take() {
      let _ = system.scheduler().with_write(|scheduler| scheduler.cancel(&handle));
    }
    if let Some(actor) = self.drive_actor.take() {
      let _ = Self::send_command(&actor, StreamDriveCommand::Shutdown);
    }
    Ok(())
  }
}
