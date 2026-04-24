//! Logging wrapper for classic FSM runtime.

use alloc::{format, string::String};
use core::{fmt::Debug, hash::Hash, time::Duration};

use super::{Fsm, FsmReason, FsmTransition};
use crate::core::kernel::{
  actor::{
    ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
  },
  event::logging::LogLevel,
};

/// Thin wrapper that emits log events around FSM transitions and termination.
pub struct LoggingFsm<State, Data>
where
  State: Clone + Debug + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static, {
  inner:       Fsm<State, Data>,
  logger_name: Option<String>,
}

impl<State, Data> LoggingFsm<State, Data>
where
  State: Clone + Debug + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Creates a new logging FSM wrapper.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: Fsm::new(), logger_name: None }
  }

  /// Configures the logger name attached to emitted events.
  #[must_use]
  pub fn with_logger_name(mut self, logger_name: impl Into<String>) -> Self {
    self.logger_name = Some(logger_name.into());
    self
  }

  /// Returns the underlying FSM runtime.
  #[must_use]
  pub const fn inner(&self) -> &Fsm<State, Data> {
    &self.inner
  }

  /// Returns mutable access to the underlying FSM runtime.
  pub const fn inner_mut(&mut self) -> &mut Fsm<State, Data> {
    &mut self.inner
  }

  /// Sets the initial state and data.
  pub fn start_with(&mut self, state: State, data: Data) {
    self.inner.start_with(state, data);
  }

  /// Registers a handler for the specified state.
  pub fn when<F>(&mut self, state: State, handler: F)
  where
    F: for<'a, 'b> FnMut(
        &mut ActorContext<'a>,
        &AnyMessageView<'b>,
        &State,
        &Data,
      ) -> Result<FsmTransition<State, Data>, ActorError>
      + Send
      + 'static, {
    self.inner.when(state, handler);
  }

  /// Registers a fallback handler used when the current state returns `unhandled`.
  pub fn when_unhandled<F>(&mut self, handler: F)
  where
    F: for<'a, 'b> FnMut(
        &mut ActorContext<'a>,
        &AnyMessageView<'b>,
        &State,
        &Data,
      ) -> Result<FsmTransition<State, Data>, ActorError>
      + Send
      + 'static, {
    self.inner.when_unhandled(handler);
  }

  /// Registers a timeout for the specified state.
  pub fn set_state_timeout(&mut self, state: State, timeout: Duration) {
    self.inner.set_state_timeout(state, timeout);
  }

  /// Starts a single-shot named timer.
  ///
  /// # Errors
  ///
  /// Returns an error when the underlying timer scheduler rejects the timer.
  pub fn start_single_timer(
    &mut self,
    ctx: &mut ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), ActorError> {
    self.inner.start_single_timer(ctx, name, message, delay)
  }

  /// Starts a named timer that fires at a fixed rate.
  ///
  /// # Errors
  ///
  /// Returns an error when the underlying timer scheduler rejects the timer.
  pub fn start_timer_at_fixed_rate(
    &mut self,
    ctx: &mut ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    interval: Duration,
  ) -> Result<(), ActorError> {
    self.inner.start_timer_at_fixed_rate(ctx, name, message, interval)
  }

  /// Starts a named timer that waits for a fixed delay after each delivery.
  ///
  /// # Errors
  ///
  /// Returns an error when the underlying timer scheduler rejects the timer.
  pub fn start_timer_with_fixed_delay(
    &mut self,
    ctx: &mut ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), ActorError> {
    self.inner.start_timer_with_fixed_delay(ctx, name, message, delay)
  }

  /// Cancels an active named timer.
  ///
  /// # Errors
  ///
  /// Returns an error when the underlying timer scheduler rejects cancellation.
  pub fn cancel_timer(&mut self, ctx: &ActorContext<'_>, name: &str) -> Result<(), ActorError> {
    self.inner.cancel_timer(ctx, name)
  }

  /// Returns `true` when a named timer is active.
  #[must_use]
  pub fn is_timer_active(&self, name: &str) -> bool {
    self.inner.is_timer_active(name)
  }

  /// Registers an observer invoked after each state transition.
  pub fn on_transition<F>(&mut self, observer: F)
  where
    F: FnMut(&State, &State) + Send + 'static, {
    self.inner.on_transition(observer);
  }

  /// Registers an observer invoked when the FSM terminates.
  pub fn on_termination<F>(&mut self, observer: F)
  where
    F: FnMut(&FsmReason, &State, &Data) + Send + 'static, {
    self.inner.on_termination(observer);
  }

  /// Arms the initial timeout and marks the FSM as initialized.
  ///
  /// # Errors
  ///
  /// Returns an error when the wrapped FSM has no initial state or timer setup fails.
  pub fn initialize(&mut self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    self.inner.initialize(ctx)
  }

  /// Evaluates the current message and emits log events for transitions and termination.
  ///
  /// # Errors
  ///
  /// Returns an error when the wrapped FSM is not initialized or the selected handler fails.
  pub fn handle(&mut self, ctx: &mut ActorContext<'_>, message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    let previous_state = self.inner.state_name().cloned();
    let was_terminated = self.inner.is_terminated();

    self.inner.handle(ctx, message)?;
    let current_state = self.inner.state_name();

    match (previous_state.as_ref(), current_state) {
      | (Some(from), Some(to)) if from != to => {
        ctx.system().emit_log(
          LogLevel::Debug,
          format!("fsm transition: {from:?} -> {to:?}"),
          Some(ctx.pid()),
          self.logger_name.clone(),
        );
      },
      | _ => {},
    }

    match (was_terminated, self.inner.is_terminated(), self.inner.last_stop_reason(), current_state) {
      | (false, true, Some(reason), Some(state)) => {
        ctx.system().emit_log(
          LogLevel::Info,
          format!("fsm terminated in state {state:?}: {reason:?}"),
          Some(ctx.pid()),
          self.logger_name.clone(),
        );
      },
      | _ => {},
    }

    Ok(())
  }

  /// Returns the current state name when initialized.
  #[must_use]
  pub const fn state_name(&self) -> Option<&State> {
    self.inner.state_name()
  }

  /// Returns the current state data when initialized.
  #[must_use]
  pub const fn state_data(&self) -> Option<&Data> {
    self.inner.state_data()
  }

  /// Returns `true` once the wrapped FSM has terminated.
  #[must_use]
  pub const fn is_terminated(&self) -> bool {
    self.inner.is_terminated()
  }
}

impl<State, Data> Default for LoggingFsm<State, Data>
where
  State: Clone + Debug + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
