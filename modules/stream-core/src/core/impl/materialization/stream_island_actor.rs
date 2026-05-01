use alloc::{format, vec::Vec};

use fraktor_actor_core_rs::core::kernel::actor::{
  Actor, ActorContext, ChildRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};

use super::{StreamIslandCommand, StreamIslandDriveGate, StreamIslandTickHandleSlot, StreamShared};
use crate::core::{StreamError, materialization::DownstreamCancellationControlPlaneShared};

#[cfg(test)]
mod tests;

/// Actor that owns and drives one stream island.
pub(crate) struct StreamIslandActor {
  stream: StreamShared,
  drive_gate: StreamIslandDriveGate,
  downstream_cancellation_control_plane: DownstreamCancellationControlPlaneShared,
  graph_streams: Vec<StreamShared>,
  tick_handle_slot: StreamIslandTickHandleSlot,
}

impl StreamIslandActor {
  pub(crate) const fn new(
    stream: StreamShared,
    drive_gate: StreamIslandDriveGate,
    downstream_cancellation_control_plane: DownstreamCancellationControlPlaneShared,
    graph_streams: Vec<StreamShared>,
    tick_handle_slot: StreamIslandTickHandleSlot,
  ) -> Self {
    Self { stream, drive_gate, downstream_cancellation_control_plane, graph_streams, tick_handle_slot }
  }

  fn cancel_scheduled_tick(&self, _ctx: &ActorContext<'_>) {
    let handle = self.tick_handle_slot.lock().take();
    let Some(handle) = handle else {
      return;
    };
    let cancelled = handle.cancel();
    if !(cancelled || handle.is_cancelled() || handle.is_completed()) {
      // Best-effort: periodic execution may already be transitioning through
      // the scheduler when the island reaches terminal state. In that case the
      // next fired tick will observe terminal state and become a no-op.
    }
  }

  fn abort_graph_streams(&self, error: &StreamError) {
    for stream in &self.graph_streams {
      stream.abort(error);
    }
  }

  fn propagate_downstream_cancellation(&self) -> Result<(), ActorError> {
    self
      .downstream_cancellation_control_plane
      .lock()
      .propagate(|upstream_actor: &mut ChildRef| {
        upstream_actor
          .try_tell(AnyMessage::new(StreamIslandCommand::Cancel { cause: None }))
          .map_err(|_| StreamError::Failed)
      })
      .map_err(|error| {
        self.abort_graph_streams(&error);
        ActorError::fatal(format!("stream island cancellation propagation failed: {error:?}"))
      })
  }

  fn drive(&self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    if self.stream.state().is_terminal() {
      self.drive_gate.mark_idle();
      self.cancel_scheduled_tick(ctx);
      return Ok(());
    }

    if let Err(error) = self.propagate_downstream_cancellation() {
      self.drive_gate.mark_idle();
      self.cancel_scheduled_tick(ctx);
      return Err(error);
    }

    let _outcome = self.stream.drive();
    self.drive_gate.mark_idle();
    if self.stream.state().is_terminal() {
      self.cancel_scheduled_tick(ctx);
    }
    Ok(())
  }

  fn cancel(&self, ctx: &ActorContext<'_>, cause: Option<&StreamError>) -> Result<(), ActorError> {
    let result = self.stream.cancel().map_err(|e| match cause {
      | Some(cause) => ActorError::fatal(format!("stream island cancel failed after {cause:?}: {e:?}")),
      | None => ActorError::fatal(format!("stream island cancel failed: {e:?}")),
    });
    self.cancel_scheduled_tick(ctx);
    result
  }

  fn shutdown(&self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    let result = self.stream.shutdown().map_err(|e| ActorError::fatal(format!("stream island shutdown failed: {e:?}")));
    if result.is_ok() {
      let _outcome = self.stream.drive();
      if self.stream.state().is_terminal() {
        self.cancel_scheduled_tick(ctx);
      }
    } else {
      self.cancel_scheduled_tick(ctx);
    }
    result
  }

  fn abort(&self, ctx: &ActorContext<'_>, error: &StreamError) -> Result<(), ActorError> {
    self.stream.abort(error);
    self.cancel_scheduled_tick(ctx);
    Err(ActorError::fatal(format!("stream island abort requested: {error:?}")))
  }
}

impl Actor for StreamIslandActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<StreamIslandCommand>() {
      match command {
        | StreamIslandCommand::Drive => {
          self.drive(ctx)?;
        },
        | StreamIslandCommand::Cancel { cause } => {
          self.cancel(ctx, cause.as_ref())?;
        },
        | StreamIslandCommand::Shutdown => {
          self.shutdown(ctx)?;
        },
        | StreamIslandCommand::Abort(error) => {
          self.abort(ctx, error)?;
        },
      }
    }
    Ok(())
  }
}
