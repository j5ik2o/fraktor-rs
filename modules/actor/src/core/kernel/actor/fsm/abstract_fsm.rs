//! Thin wrapper matching classic AbstractFSM naming.

use core::{hash::Hash, time::Duration};

use super::{Fsm, FsmReason, FsmTransition};
use crate::core::kernel::actor::{ActorContext, error::ActorError, messaging::AnyMessageView};

/// Thin wrapper over [`Fsm`] matching classic `AbstractFSM` naming.
pub struct AbstractFsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static, {
  inner: Fsm<State, Data>,
}

impl<State, Data> AbstractFsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Creates a new wrapper around an empty FSM runtime.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: Fsm::new() }
  }

  /// Returns the underlying FSM runtime.
  #[must_use]
  pub const fn inner(&self) -> &Fsm<State, Data> {
    &self.inner
  }

  /// Returns mutable access to the underlying FSM runtime.
  #[must_use]
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

  /// Evaluates the current message against the active state handler.
  ///
  /// # Errors
  ///
  /// Returns an error when the wrapped FSM is not initialized or the selected handler fails.
  pub fn handle(&mut self, ctx: &mut ActorContext<'_>, message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    self.inner.handle(ctx, message)
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

impl<State, Data> Default for AbstractFsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
