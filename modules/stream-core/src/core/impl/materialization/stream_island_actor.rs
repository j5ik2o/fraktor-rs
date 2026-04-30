use alloc::format;

use fraktor_actor_core_rs::core::kernel::actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView};

use super::{StreamIslandCommand, StreamIslandDriveGate, StreamShared};
use crate::core::StreamError;

#[cfg(test)]
mod tests;

/// Actor that owns and drives one stream island.
pub(crate) struct StreamIslandActor {
  stream:     StreamShared,
  drive_gate: StreamIslandDriveGate,
}

impl StreamIslandActor {
  pub(crate) const fn new(stream: StreamShared, drive_gate: StreamIslandDriveGate) -> Self {
    Self { stream, drive_gate }
  }

  fn drive(&self) {
    if self.stream.state().is_terminal() {
      self.drive_gate.mark_idle();
      return;
    }

    let _outcome = self.stream.drive();
    self.drive_gate.mark_idle();
  }

  fn cancel(&self, cause: Option<&StreamError>) -> Result<(), ActorError> {
    self.stream.cancel().map_err(|e| match cause {
      | Some(cause) => ActorError::fatal(format!("stream island cancel failed after {cause:?}: {e:?}")),
      | None => ActorError::fatal(format!("stream island cancel failed: {e:?}")),
    })
  }

  fn shutdown(&self) -> Result<(), ActorError> {
    self.stream.shutdown().map_err(|e| ActorError::fatal(format!("stream island shutdown failed: {e:?}")))?;
    let _outcome = self.stream.drive();
    Ok(())
  }

  fn abort(&self, error: &StreamError) -> Result<(), ActorError> {
    self.stream.abort(error);
    Err(ActorError::fatal(format!("stream island abort requested: {error:?}")))
  }
}

impl Actor for StreamIslandActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StreamIslandCommand>() {
      match command {
        | StreamIslandCommand::Drive => {
          self.drive();
        },
        | StreamIslandCommand::Cancel { cause } => {
          self.cancel(cause.as_ref())?;
        },
        | StreamIslandCommand::Shutdown => {
          self.shutdown()?;
        },
        | StreamIslandCommand::Abort(error) => {
          self.abort(error)?;
        },
      }
    }
    Ok(())
  }
}
