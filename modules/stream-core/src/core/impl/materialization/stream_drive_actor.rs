use alloc::{collections::BTreeMap, format, vec::Vec};

use fraktor_actor_core_rs::core::kernel::actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView};

use super::{StreamDriveCommand, StreamShared};

/// Actor that drives registered streams on each tick.
pub(crate) struct StreamDriveActor {
  streams:            BTreeMap<u64, StreamShared>,
  shutdown_requested: bool,
}

impl StreamDriveActor {
  pub(crate) const fn new() -> Self {
    Self { streams: BTreeMap::new(), shutdown_requested: false }
  }

  fn register(&mut self, stream: StreamShared) -> Result<(), ActorError> {
    if self.shutdown_requested {
      stream.cancel().map_err(|e| ActorError::fatal(format!("cancel during shutdown-register failed: {e:?}")))?;
      return Ok(());
    }
    self.streams.insert(stream.id(), stream);
    Ok(())
  }

  fn tick(&mut self) {
    let mut finished = Vec::new();
    for (id, stream) in self.streams.iter() {
      // outcome is observed indirectly via stream.state().is_terminal() below
      let _outcome = stream.drive();
      if stream.state().is_terminal() {
        finished.push(*id);
      }
    }
    for id in finished {
      self.streams.remove(&id);
    }
  }

  fn shutdown(&mut self) -> Result<(), ActorError> {
    let mut failed_ids = Vec::new();
    for (id, stream) in self.streams.iter() {
      if let Err(e) = stream.cancel() {
        failed_ids.push((*id, e));
      }
    }
    if failed_ids.is_empty() {
      self.streams.clear();
      self.shutdown_requested = true;
      Ok(())
    } else {
      // Remove only successfully cancelled streams; keep failed ones for retry
      let failed_set: Vec<u64> = failed_ids.iter().map(|(id, _)| *id).collect();
      self.streams.retain(|id, _| failed_set.contains(id));
      self.shutdown_requested = true;
      let msg = failed_ids.iter().map(|(id, e)| format!("stream {id:?}: {e:?}")).collect::<Vec<_>>().join(", ");
      Err(ActorError::fatal(format!("stream cancel failed: [{msg}]")))
    }
  }

  fn try_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if self.shutdown_requested && self.streams.is_empty() {
      ctx.stop_self().map_err(|error| ActorError::from_send_error(&error))
    } else {
      Ok(())
    }
  }
}

impl Actor for StreamDriveActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StreamDriveCommand>() {
      match command {
        | StreamDriveCommand::Register { stream } => {
          self.register(stream.clone())?;
        },
        | StreamDriveCommand::Tick => {
          self.tick();
        },
        | StreamDriveCommand::Shutdown => {
          self.shutdown()?;
        },
      }
    }
    self.try_stop(ctx)
  }
}
