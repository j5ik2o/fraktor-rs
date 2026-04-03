//! Minimal classic FSM runtime.

use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{hash::Hash, time::Duration};

use ahash::RandomState;
use hashbrown::HashMap;
use portable_atomic::{AtomicU64, Ordering};

use super::{FsmReason, FsmStateTimeout, FsmTransition};
use crate::core::kernel::actor::{
  ActorContext,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  scheduler::SchedulerError,
};

type StateHandler<State, Data> = dyn for<'a, 'b> FnMut(
    &mut ActorContext<'a>,
    &AnyMessageView<'b>,
    &State,
    &Data,
  ) -> Result<FsmTransition<State, Data>, ActorError>
  + Send
  + 'static;

type TransitionObserver<State> = dyn FnMut(&State, &State) + Send + 'static;
type TerminationObserver<State, Data> = dyn FnMut(&FsmReason, &State, &Data) + Send + 'static;

static FSM_TIMER_KEY_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Minimal classic FSM runtime for untyped actors.
pub struct Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static, {
  state:                 Option<State>,
  data:                  Option<Data>,
  handlers:              HashMap<State, Box<StateHandler<State, Data>>, RandomState>,
  unhandled_handler:     Option<Box<StateHandler<State, Data>>>,
  transition_observers:  Vec<Box<TransitionObserver<State>>>,
  termination_observers: Vec<Box<TerminationObserver<State, Data>>>,
  state_timeouts:        HashMap<State, Duration, RandomState>,
  initialized:           bool,
  terminated:            bool,
  last_stop_reason:      Option<FsmReason>,
  timeout_generation:    u64,
  timer_key:             String,
}

impl<State, Data> Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Creates a new empty FSM runtime.
  #[must_use]
  pub fn new() -> Self {
    let timer_key = format!("fraktor-fsm-timeout-{}", FSM_TIMER_KEY_COUNTER.fetch_add(1, Ordering::Relaxed));
    Self {
      state: None,
      data: None,
      handlers: HashMap::with_hasher(RandomState::new()),
      unhandled_handler: None,
      transition_observers: Vec::new(),
      termination_observers: Vec::new(),
      state_timeouts: HashMap::with_hasher(RandomState::new()),
      initialized: false,
      terminated: false,
      last_stop_reason: None,
      timeout_generation: 0,
      timer_key,
    }
  }

  /// Sets the initial state and data.
  pub fn start_with(&mut self, state: State, data: Data) {
    self.state = Some(state);
    self.data = Some(data);
    self.initialized = false;
    self.terminated = false;
    self.last_stop_reason = None;
  }

  /// Registers a handler for the specified state.
  ///
  /// # Panics
  ///
  /// Panics if a handler has already been registered for the same state.
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
    let previous = self.handlers.insert(state, Box::new(handler));
    assert!(previous.is_none(), "Fsm: duplicate state handler registered");
  }

  /// Registers a fallback handler invoked when the current state handler returns `unhandled`.
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
    self.unhandled_handler = Some(Box::new(handler));
  }

  /// Registers a timeout for the specified state.
  ///
  /// # Panics
  ///
  /// Panics when `timeout` is zero.
  pub fn set_state_timeout(&mut self, state: State, timeout: Duration) {
    assert!(!timeout.is_zero(), "Fsm: state timeout must be positive");
    self.state_timeouts.insert(state, timeout);
  }

  /// Registers an observer invoked when the FSM changes state.
  pub fn on_transition<F>(&mut self, observer: F)
  where
    F: FnMut(&State, &State) + Send + 'static, {
    self.transition_observers.push(Box::new(observer));
  }

  /// Registers an observer invoked when the FSM terminates.
  pub fn on_termination<F>(&mut self, observer: F)
  where
    F: FnMut(&FsmReason, &State, &Data) + Send + 'static, {
    self.termination_observers.push(Box::new(observer));
  }

  /// Arms the initial state timeout and marks the FSM as active.
  ///
  /// # Errors
  ///
  /// Returns an error when the initial state has not been configured or timer registration fails.
  pub fn initialize(&mut self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    if self.state.is_none() || self.data.is_none() {
      return Err(ActorError::recoverable("fsm requires start_with before initialize"));
    }
    self.reschedule_state_timeout(ctx)?;
    self.initialized = true;
    self.terminated = false;
    self.last_stop_reason = None;
    Ok(())
  }

  /// Evaluates the current message against the active state handler.
  ///
  /// # Errors
  ///
  /// Returns an error when the FSM is not initialized or when the selected handler fails.
  pub fn handle(&mut self, ctx: &mut ActorContext<'_>, message: &AnyMessageView<'_>) -> Result<(), ActorError> {
    if !self.initialized {
      return Err(ActorError::recoverable("fsm not initialized"));
    }
    if self.terminated {
      return Ok(());
    }

    if self.is_stale_timeout(message) {
      return Ok(());
    }

    let current_state = self.state.clone().ok_or_else(|| ActorError::recoverable("fsm has no current state"))?;
    let current_data = self.data.clone().ok_or_else(|| ActorError::recoverable("fsm has no current data"))?;

    let mut transition = match self.handlers.get_mut(&current_state) {
      | Some(handler) => handler(ctx, message, &current_state, &current_data)?,
      | None => FsmTransition::unhandled(),
    };

    if !transition.handled()
      && let Some(handler) = self.unhandled_handler.as_mut()
    {
      transition = handler(ctx, message, &current_state, &current_data)?;
    }

    if !transition.handled() {
      return Ok(());
    }

    self.apply_transition(ctx, &current_state, current_data, transition)
  }

  /// Returns the current state name when initialized.
  #[must_use]
  pub const fn state_name(&self) -> Option<&State> {
    self.state.as_ref()
  }

  /// Returns the current state data when initialized.
  #[must_use]
  pub const fn state_data(&self) -> Option<&Data> {
    self.data.as_ref()
  }

  /// Returns `true` once the FSM has been stopped.
  #[must_use]
  pub const fn is_terminated(&self) -> bool {
    self.terminated
  }

  /// Returns the current timeout generation used to reject stale timeout messages.
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.timeout_generation
  }

  /// Returns the last stop reason, if any.
  #[must_use]
  pub const fn last_stop_reason(&self) -> Option<&FsmReason> {
    self.last_stop_reason.as_ref()
  }

  fn apply_transition(
    &mut self,
    ctx: &ActorContext<'_>,
    current_state: &State,
    current_data: Data,
    transition: FsmTransition<State, Data>,
  ) -> Result<(), ActorError> {
    let (next_state, next_data, stop_reason) = transition.into_parts();
    let next_data = next_data.unwrap_or(current_data);

    if let Some(reason) = stop_reason {
      self.cancel_state_timeout(ctx)?;
      self.data = Some(next_data.clone());
      self.terminated = true;
      self.last_stop_reason = Some(reason.clone());
      for observer in &mut self.termination_observers {
        observer(&reason, current_state, &next_data);
      }
      return Ok(());
    }

    let next_state = next_state.unwrap_or_else(|| current_state.clone());
    let state_changed = next_state != *current_state;

    if state_changed {
      self.reschedule_state_timeout_for_state(ctx, &next_state)?;
    }

    self.state = Some(next_state.clone());
    self.data = Some(next_data);

    if state_changed {
      for observer in &mut self.transition_observers {
        observer(current_state, &next_state);
      }
    }

    Ok(())
  }

  fn reschedule_state_timeout(&mut self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    let Some(state) = self.state.clone() else {
      return Ok(());
    };
    self.reschedule_state_timeout_for_state(ctx, &state)
  }

  fn reschedule_state_timeout_for_state(&mut self, ctx: &ActorContext<'_>, state: &State) -> Result<(), ActorError> {
    self.cancel_state_timeout(ctx)?;
    let Some(timeout) = self.state_timeouts.get(state).copied() else {
      return Ok(());
    };
    self.timeout_generation = self.timeout_generation.wrapping_add(1);
    let message = AnyMessage::new(FsmStateTimeout::new(state.clone(), self.timeout_generation));
    ctx
      .timers()
      .start_single_timer(self.timer_key.clone(), message, timeout)
      .map_err(|error| Self::scheduler_error_to_actor_error(&error))
  }

  fn cancel_state_timeout(&self, ctx: &ActorContext<'_>) -> Result<(), ActorError> {
    ctx.timers().cancel(&self.timer_key).map_err(|error| Self::scheduler_error_to_actor_error(&error))
  }

  fn is_stale_timeout(&self, message: &AnyMessageView<'_>) -> bool {
    let Some(timeout) = message.downcast_ref::<FsmStateTimeout<State>>() else {
      return false;
    };
    let Some(current_state) = self.state.as_ref() else {
      return true;
    };
    timeout.generation() != self.timeout_generation || timeout.state() != current_state
  }

  fn scheduler_error_to_actor_error(error: &SchedulerError) -> ActorError {
    ActorError::recoverable_typed::<SchedulerError>(format!("fsm timer operation failed: {error:?}"))
  }
}

impl<State, Data> Default for Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
