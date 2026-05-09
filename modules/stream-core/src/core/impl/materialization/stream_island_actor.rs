use alloc::{format, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext, ChildRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};
use fraktor_utils_core_rs::core::sync::SharedAccess;

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
      // ベストエフォート: island が terminal に到達する瞬間は scheduler 側の
      // 遷移と競合し得る。その場合は次に発火した tick が terminal 状態を観測して
      // no-op になる。
    }
  }

  fn abort_graph_streams(&self, error: &StreamError) {
    for stream in &self.graph_streams {
      let kill_switch_state = stream.with_read(|stream| stream.kill_switch_state());
      if let Some((_abort_error, command_targets)) = kill_switch_state.lock().request_abort(error.clone()) {
        // The failure path already aborts every graph stream directly below; re-sending
        // actor abort commands through the returned targets would duplicate delivery.
        drop(command_targets);
      }
      stream.abort(error);
    }
  }

  fn propagate_downstream_cancellation(&self) -> Result<(), ActorError> {
    // Fast path: skip the inner mutex entirely when no boundary has signalled
    // a downstream cancel since the previous propagation cycle.
    if !self.downstream_cancellation_control_plane.take_pending() {
      return Ok(());
    }

    let targets = self
      .downstream_cancellation_control_plane
      .with_locked(|control_plane| control_plane.reserve_cancellation_targets());
    if targets.is_empty() {
      return Ok(());
    }

    let mut delivery_results = Vec::with_capacity(targets.len());
    let mut first_error = None;

    for target in targets {
      let actor_pid = target.actor_pid();
      let mut upstream_actor: ChildRef = target.into_actor();
      let delivered = upstream_actor.try_tell(AnyMessage::new(StreamIslandCommand::Cancel { cause: None })).is_ok();
      if !delivered && first_error.is_none() {
        first_error = Some(StreamError::Failed);
      }
      delivery_results.push((actor_pid, delivered));
    }

    self.downstream_cancellation_control_plane.with_locked(|control_plane| {
      for (actor_pid, delivered) in delivery_results {
        control_plane.finish_cancellation_delivery(actor_pid, delivered);
      }
    });

    if let Some(error) = first_error {
      // A failed delivery aborts the whole graph below, after which every
      // island enters a terminal state and `drive` short-circuits before
      // ever reaching the propagator again. Re-arming the pending latch is
      // therefore unnecessary: on success, routes with delivered==true have
      // their cancel_command_count incremented and become non-reservable,
      // so the next drive's fast path correctly stays out of the lock.
      self.abort_graph_streams(&error);
      return Err(ActorError::fatal(format!("stream island cancellation propagation failed: {error:?}")));
    }

    Ok(())
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
