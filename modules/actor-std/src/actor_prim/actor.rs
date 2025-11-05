use cellactor_actor_core_rs::{actor_prim::Pid, error::ActorError};

use super::ActorContext;
use crate::messaging::AnyMessageView;

/// Defines the lifecycle contract for actors executed with `StdToolbox`.
pub trait Actor: Send {
  /// Called once before the actor starts processing messages.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor fails to initialize and should not start.
  ///
  /// # Panics
  ///
  /// Panics are not expected. Implementations should return `Err` so the
  /// supervisor can decide how to recover.
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Handles a single incoming message dispatched to this actor instance.
  ///
  /// # Errors
  ///
  /// Returns an error to signal recoverable or fatal processing failures.
  ///
  /// # Panics
  ///
  /// Panics are considered fatal and will propagate to the runtime.
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError>;

  /// Called once after the actor has been stopped.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup work fails.
  ///
  /// # Panics
  ///
  /// Panics are not expected. Implementations should return `Err` to allow
  /// supervisor policies to react.
  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a watched actor terminates.
  ///
  /// # Errors
  ///
  /// Returns an error when the hook fails to handle the termination event.
  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, _terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }
}
