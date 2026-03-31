use alloc::{collections::BTreeMap, format, vec::Vec};

use fraktor_actor_rs::core::kernel::actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView};

use super::{StreamHandleId, StreamHandleImpl, stream_runtime_completion::StreamDriveCommand};

/// Actor that drives registered streams on each tick.
pub(crate) struct StreamDriveActor {
  handles:            BTreeMap<StreamHandleId, StreamHandleImpl>,
  shutdown_requested: bool,
}

impl StreamDriveActor {
  pub(crate) const fn new() -> Self {
    Self { handles: BTreeMap::new(), shutdown_requested: false }
  }

  fn register(&mut self, handle: StreamHandleImpl) -> Result<(), ActorError> {
    if self.shutdown_requested {
      handle.cancel().map_err(|e| ActorError::fatal(format!("cancel during shutdown-register failed: {e:?}")))?;
      return Ok(());
    }
    self.handles.insert(handle.id(), handle);
    Ok(())
  }

  fn tick(&mut self) {
    let mut finished = Vec::new();
    for (id, handle) in self.handles.iter() {
      // outcome is observed indirectly via handle.state().is_terminal() below
      let _outcome = handle.drive();
      if handle.state().is_terminal() {
        finished.push(*id);
      }
    }
    for id in finished {
      self.handles.remove(&id);
    }
  }

  fn shutdown(&mut self) -> Result<(), ActorError> {
    let mut failed_ids = Vec::new();
    for (id, handle) in self.handles.iter() {
      if let Err(e) = handle.cancel() {
        failed_ids.push((*id, e));
      }
    }
    if failed_ids.is_empty() {
      self.handles.clear();
      self.shutdown_requested = true;
      Ok(())
    } else {
      // Remove only successfully cancelled handles; keep failed ones for retry
      let failed_set: Vec<StreamHandleId> = failed_ids.iter().map(|(id, _)| *id).collect();
      self.handles.retain(|id, _| failed_set.contains(id));
      self.shutdown_requested = true;
      let msg = failed_ids.iter().map(|(id, e)| format!("handle {id:?}: {e:?}")).collect::<Vec<_>>().join(", ");
      Err(ActorError::fatal(format!("stream cancel failed: [{msg}]")))
    }
  }

  fn try_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if self.shutdown_requested && self.handles.is_empty() {
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
        | StreamDriveCommand::Register { handle } => {
          self.register(handle.clone())?;
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
