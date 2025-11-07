use cellactor_actor_core_rs::{actor_prim::Pid, error::ActorError, supervision::SupervisorStrategy};

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
  /// use core::time::Duration;
  ///
  /// use cellactor_actor_core_rs::{
  ///   error::ActorError,
  ///   supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  /// };
  /// use cellactor_actor_std_rs::{
  ///   actor_prim::{Actor, ActorContext},
  ///   messaging::AnyMessageView,
  /// };
  /// struct ResilientWorker {
  ///   consecutive_errors: u32,
  /// }
  ///
  /// impl Actor for ResilientWorker {
  ///   fn receive(
  ///     &mut self,
  ///     _ctx: &mut ActorContext<'_>,
  ///     _message: AnyMessageView<'_>,
  ///   ) -> Result<(), ActorError> {
  ///     Ok(())
  ///   }
  ///
  ///   fn supervisor_strategy(&mut self, _ctx: &mut ActorContext) -> SupervisorStrategy {
  ///     if self.consecutive_errors > 10 {
  ///       // Too many errors: stop immediately
  ///       SupervisorStrategy::new(
  ///         SupervisorStrategyKind::OneForOne,
  ///         0,
  ///         Duration::from_secs(0),
  ///         |_| SupervisorDirective::Stop,
  ///       )
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
  fn supervisor_strategy(&mut self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategy {
    SupervisorStrategy::default()
  }
}
