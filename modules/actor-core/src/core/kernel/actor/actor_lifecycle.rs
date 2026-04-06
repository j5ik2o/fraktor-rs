//! Actor lifecycle contract.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use crate::core::kernel::{
  actor::{ActorContext, Pid, error::ActorError, messaging::AnyMessageView, supervision::SupervisorStrategyConfig},
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
};

/// Defines the lifecycle hooks that every actor must implement.
pub trait Actor: Send {
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

  /// Called when a watched actor terminates and notifies this actor via DeathWatch.
  ///
  /// # Errors
  ///
  /// Returns an error when cleanup logic fails.
  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, _terminated: Pid) -> Result<(), ActorError> {
    Ok(())
  }

  /// Called when this actor's mailbox reaches high pressure while processing inbound traffic.
  ///
  /// # Errors
  ///
  /// Returns an error when the actor cannot react to pressure conditions.
  fn on_mailbox_pressure(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  /// Provides the supervision strategy for child actors.
  ///
  /// This method allows actors to dynamically determine supervision behavior based on
  /// their internal state. The returned strategy controls how child actor failures are handled.
  ///
  /// # Default Implementation
  ///
  /// Returns `SupervisorStrategyConfig::default()` which provides a conservative restart policy:
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
  /// use fraktor_actor_core_rs::core::kernel::actor::{
  ///   Actor, ActorContext,
  ///   error::ActorError,
  ///   messaging::AnyMessageView,
  ///   supervision::{
  ///     SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind,
  ///   },
  /// };
  /// use fraktor_utils_rs::core::sync::NoStdMutex;
  ///
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
  ///   fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
  ///     if self.consecutive_errors > 10 {
  ///       // Too many errors: stop immediately
  ///       SupervisorStrategy::new(
  ///         SupervisorStrategyKind::OneForOne,
  ///         0,
  ///         Duration::from_secs(0),
  ///         |_| SupervisorDirective::Stop,
  ///       )
  ///       .into()
  ///     } else {
  ///       // Normal operation: allow retries
  ///       SupervisorStrategyConfig::default()
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
  /// - **Must not mutate actor state**: This method is queried while coordinating supervision.
  ///   `ctx` remains mutable so implementations can inspect runtime state or emit logs, but they
  ///   must not update actor-owned state or perform coordination-affecting side effects here.
  ///
  /// # See Also
  ///
  /// - [`SupervisorStrategyConfig`] for available strategies
  /// - [`SupervisorDirective`](crate::core::kernel::actor::supervision::SupervisorDirective) for
  ///   failure handling options
  #[must_use]
  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategyConfig::default()
  }

  /// Called before the actor is restarted by its supervisor.
  ///
  /// The default implementation delegates to [`post_stop`](Actor::post_stop).
  ///
  /// # Errors
  ///
  /// Returns an error when pre-restart cleanup fails.
  fn pre_restart(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.post_stop(ctx)
  }

  /// Called when a supervised child actor fails.
  ///
  /// # Errors
  ///
  /// Returns an error when the notification cannot be processed.
  fn on_child_failed(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _child: Pid,
    _error: &ActorError,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

impl<T> Actor for Box<T>
where
  T: Actor + ?Sized,
{
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    (**self).pre_start(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    (**self).receive(ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    (**self).post_stop(ctx)
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    (**self).on_terminated(ctx, terminated)
  }

  fn on_mailbox_pressure(
    &mut self,
    ctx: &mut ActorContext<'_>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    (**self).on_mailbox_pressure(ctx, event)
  }

  fn supervisor_strategy(&self, ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    (**self).supervisor_strategy(ctx)
  }

  fn pre_restart(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    (**self).pre_restart(ctx)
  }

  fn on_child_failed(&mut self, ctx: &mut ActorContext<'_>, child: Pid, error: &ActorError) -> Result<(), ActorError> {
    (**self).on_child_failed(ctx, child, error)
  }
}
