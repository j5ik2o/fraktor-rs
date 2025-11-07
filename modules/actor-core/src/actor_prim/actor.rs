//! Actor lifecycle contract.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  actor_prim::{ActorContextGeneric, Pid},
  error::ActorError,
  messaging::AnyMessageView,
  supervision::SupervisorStrategy,
};

/// Defines the lifecycle hooks that every actor must implement.
pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send {
  /// Called once before the actor starts processing messages.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor fails to initialize and should not start.
  ///
  /// # Panics
  ///
  /// Panics are not expected. Implementations should return `Err` instead so the
  /// supervisor can decide how to recover.
  fn pre_start(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
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
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageView<'_, TB>,
  ) -> Result<(), ActorError>;

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
  fn post_stop(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when a watched actor terminates and notifies this actor via DeathWatch.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup logic fails.
  fn on_terminated(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>, _terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }

  /// Provides the supervision strategy for child actors.
  ///
  /// This method allows actors to dynamically determine supervision behavior based on
  /// their internal state. The returned strategy controls how child actor failures are handled.
  ///
  /// # Default Implementation
  ///
  /// Returns `SupervisorStrategy::default()` which provides a conservative restart policy:
  /// - Strategy kind: OneForOne (only restart the failed child)
  /// - Maximum restarts: 10 times
  /// - Time window: 1 second
  /// - Decider: Restart on recoverable errors, Stop on fatal errors
  ///
  /// # Customization
  ///
  /// Override this method to provide dynamic supervision based on actor state.
  /// The `ctx` parameter allows access to system configuration and logging.
  ///
  /// # Examples
  ///
  /// ```
  /// struct ResilientWorker {
  ///   consecutive_errors: u32,
  /// }
  ///
  /// impl Actor for ResilientWorker {
  ///   fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
  ///     if self.consecutive_errors > 10 {
  ///       // Too many errors: stop immediately
  ///       SupervisorStrategy::stopping()
  ///     } else {
  ///       // Normal operation: allow retries
  ///       SupervisorStrategy::default()
  ///     }
  ///   }
  /// }
  /// ```
  ///
  /// # Implementation Requirements
  ///
  /// - **Must be panic-free**: This method is called during failure handling. Panics will cause
  ///   system instability or termination (especially in no_std environments).
  /// - **Should be lightweight**: Called on every child failure, though failures are infrequent.
  /// - **May update state**: The `&mut self` receiver allows state updates (e.g., error counters).
  ///
  /// # See Also
  ///
  /// - [`SupervisorStrategy`] for available strategies
  /// - [`SupervisorDirective`] for failure handling options
  #[must_use]
  fn supervisor_strategy(&mut self, _ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
    SupervisorStrategy::default()
  }
}

impl<T, TB> Actor<TB> for Box<T>
where
  T: Actor<TB> + ?Sized,
  TB: RuntimeToolbox,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    (**self).pre_start(ctx)
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageView<'_, TB>,
  ) -> Result<(), ActorError> {
    (**self).receive(ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    (**self).post_stop(ctx)
  }

  fn on_terminated(&mut self, ctx: &mut ActorContextGeneric<'_, TB>, terminated: Pid) -> Result<(), ActorError> {
    (**self).on_terminated(ctx, terminated)
  }

  fn supervisor_strategy(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> SupervisorStrategy {
    (**self).supervisor_strategy(ctx)
  }
}
