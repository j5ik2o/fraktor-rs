//! Minimal classic FSM runtime.

use alloc::{boxed::Box, format, string::String, vec::Vec};
use core::{hash::Hash, time::Duration};

use ahash::RandomState;
use hashbrown::HashMap;
use portable_atomic::{AtomicU64, Ordering};

use super::{FsmReason, FsmStateTimeout, FsmTimerFired, FsmTransition, fsm_named_timer::FsmNamedTimer};
use crate::{
  actor::{
    ActorContext,
    error::{ActorError, ActorErrorReason},
    messaging::{AnyMessage, AnyMessageView},
    scheduler::SchedulerError,
  },
  event::logging::LogLevel,
};

#[cfg(test)]
mod tests;

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

enum NamedTimerOutcome {
  NotTimer,
  Stale,
  Deliver(AnyMessage),
}

static FSM_TIMER_KEY_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Minimal classic FSM runtime for untyped actors.
pub struct Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static, {
  state:                  Option<State>,
  data:                   Option<Data>,
  handlers:               HashMap<State, Box<StateHandler<State, Data>>, RandomState>,
  unhandled_handler:      Option<Box<StateHandler<State, Data>>>,
  transition_observers:   Vec<Box<TransitionObserver<State>>>,
  termination_observers:  Vec<Box<TerminationObserver<State, Data>>>,
  state_timeouts:         HashMap<State, Duration, RandomState>,
  initialized:            bool,
  terminated:             bool,
  last_stop_reason:       Option<FsmReason>,
  timeout_generation:     u64,
  timer_key:              String,
  named_timers:           HashMap<String, FsmNamedTimer, RandomState>,
  named_timer_generation: u64,
  named_timer_key_prefix: String,
}

impl<State, Data> Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Creates a new empty FSM runtime.
  #[must_use]
  pub fn new() -> Self {
    let timer_id = FSM_TIMER_KEY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timer_key = format!("fraktor-fsm-timeout-{timer_id}");
    let named_timer_key_prefix = format!("fraktor-fsm-named-{timer_id}");
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
      named_timers: HashMap::with_hasher(RandomState::new()),
      named_timer_generation: 0,
      named_timer_key_prefix,
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

  /// Starts a one-shot named timer that delivers `message` back to this FSM.
  ///
  /// Reusing the same `name` cancels the previous timer and discards any late
  /// arrival from the replaced generation.
  ///
  /// # Errors
  ///
  /// Returns an error when the scheduler rejects the timer registration.
  /// If registration fails after replacing an existing `name`, the previous timer
  /// has already been cancelled and removed.
  pub fn start_single_timer(
    &mut self,
    ctx: &ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), ActorError> {
    self.start_named_timer(ctx, name, message, false, |timer_key, fired| {
      ctx.timers().start_single_timer(timer_key, fired, delay)
    })
  }

  /// Starts a fixed-rate named timer that delivers `message` back to this FSM.
  ///
  /// Reusing the same `name` cancels the previous timer and discards any late
  /// arrival from the replaced generation.
  ///
  /// # Errors
  ///
  /// Returns an error when the scheduler rejects the timer registration.
  /// If registration fails after replacing an existing `name`, the previous timer
  /// has already been cancelled and removed.
  pub fn start_timer_at_fixed_rate(
    &mut self,
    ctx: &ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    interval: Duration,
  ) -> Result<(), ActorError> {
    self.start_named_timer(ctx, name, message, true, |timer_key, fired| {
      ctx.timers().start_timer_at_fixed_rate(timer_key, fired, interval)
    })
  }

  /// Starts a fixed-delay named timer that delivers `message` back to this FSM.
  ///
  /// Reusing the same `name` cancels the previous timer and discards any late
  /// arrival from the replaced generation.
  ///
  /// # Errors
  ///
  /// Returns an error when the scheduler rejects the timer registration.
  /// If registration fails after replacing an existing `name`, the previous timer
  /// has already been cancelled and removed.
  pub fn start_timer_with_fixed_delay(
    &mut self,
    ctx: &ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    delay: Duration,
  ) -> Result<(), ActorError> {
    self.start_named_timer(ctx, name, message, true, |timer_key, fired| {
      ctx.timers().start_timer_with_fixed_delay(timer_key, fired, delay)
    })
  }

  /// Cancels the named timer if it is active.
  ///
  /// # Errors
  ///
  /// Returns an error when the scheduler cannot cancel the registered timer.
  pub fn cancel_timer(&mut self, ctx: &ActorContext<'_>, name: &str) -> Result<(), ActorError> {
    let Some(timer) = self.named_timers.remove(name) else {
      return Ok(());
    };
    ctx.timers().cancel(timer.timer_key()).map_err(|error| Self::scheduler_error_to_actor_error(&error))
  }

  /// Returns whether the named timer is currently active from the FSM perspective.
  #[must_use]
  pub fn is_timer_active(&self, name: &str) -> bool {
    self.named_timers.contains_key(name)
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

    let named_timer_outcome = self.take_named_timer_payload(message);
    let payload_view;
    let message = match &named_timer_outcome {
      | NamedTimerOutcome::Deliver(payload) => {
        payload_view = payload.as_view();
        &payload_view
      },
      | NamedTimerOutcome::Stale => return Ok(()),
      | NamedTimerOutcome::NotTimer => {
        if self.is_stale_timeout(message) {
          return Ok(());
        }
        message
      },
    };

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
    ctx: &mut ActorContext<'_>,
    current_state: &State,
    current_data: Data,
    mut transition: FsmTransition<State, Data>,
  ) -> Result<(), ActorError> {
    let for_max_timeout = transition.for_max_timeout();
    let replies = transition.take_replies();
    let (next_state, next_data, stop_reason) = transition.into_parts();
    let next_data = next_data.unwrap_or(current_data);

    if let Some(reason) = stop_reason {
      self.cancel_state_timeout(ctx)?;
      self.data = Some(next_data.clone());
      self.terminated = true;
      self.last_stop_reason = Some(reason.clone());
      Self::dispatch_replies(ctx, replies);
      for observer in &mut self.termination_observers {
        observer(&reason, current_state, &next_data);
      }
      self.cancel_all_named_timers_best_effort(ctx);
      return Ok(());
    }

    let explicit_transition = next_state.is_some();
    let next_state = next_state.unwrap_or_else(|| current_state.clone());

    self.state = Some(next_state.clone());
    self.data = Some(next_data);

    if explicit_transition {
      for observer in &mut self.transition_observers {
        observer(current_state, &next_state);
      }
    }

    Self::dispatch_replies(ctx, replies);
    self.apply_for_max_timeout(ctx, &next_state, explicit_transition, for_max_timeout)?;

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
    self.schedule_state_timeout_for_duration(ctx, state, timeout)
  }

  fn schedule_state_timeout_for_duration(
    &mut self,
    ctx: &ActorContext<'_>,
    state: &State,
    timeout: Duration,
  ) -> Result<(), ActorError> {
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

  fn apply_for_max_timeout(
    &mut self,
    ctx: &ActorContext<'_>,
    state: &State,
    explicit_transition: bool,
    for_max_timeout: Option<Option<Duration>>,
  ) -> Result<(), ActorError> {
    match for_max_timeout {
      | Some(Some(timeout)) => {
        self.cancel_state_timeout(ctx)?;
        self.schedule_state_timeout_for_duration(ctx, state, timeout)
      },
      | Some(None) => {
        self.cancel_state_timeout(ctx)?;
        self.timeout_generation = self.timeout_generation.wrapping_add(1);
        Ok(())
      },
      | None if explicit_transition => self.reschedule_state_timeout_for_state(ctx, state),
      | None => Ok(()),
    }
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

  fn take_named_timer_payload(&mut self, message: &AnyMessageView<'_>) -> NamedTimerOutcome {
    let Some(fired) = message.downcast_ref::<FsmTimerFired>() else {
      return NamedTimerOutcome::NotTimer;
    };
    let is_repeating = {
      let Some(timer) = self.named_timers.get(fired.name()) else {
        return NamedTimerOutcome::Stale;
      };
      if timer.generation() != fired.generation() {
        return NamedTimerOutcome::Stale;
      }
      timer.is_repeating()
    };

    let payload = fired.payload().clone();
    if !is_repeating {
      let removed = self.named_timers.remove(fired.name());
      debug_assert!(removed.is_some());
    }
    NamedTimerOutcome::Deliver(payload)
  }

  fn dispatch_replies(ctx: &mut ActorContext<'_>, replies: Vec<AnyMessage>) {
    for reply in replies {
      if let Err(error) = ctx.reply(reply) {
        ctx.system().state().record_send_error(None, &error);
      }
    }
  }

  fn cancel_replaced_named_timer(&mut self, ctx: &ActorContext<'_>, name: &str) {
    if let Some(timer) = self.named_timers.remove(name)
      && let Err(error) = ctx.timers().cancel(timer.timer_key())
    {
      Self::log_scheduler_warning(ctx, format!("fsm named timer replacement cancel failed for {name}: {error:?}"));
    }
  }

  fn start_named_timer<F>(
    &mut self,
    ctx: &ActorContext<'_>,
    name: impl Into<String>,
    message: AnyMessage,
    is_repeating: bool,
    schedule: F,
  ) -> Result<(), ActorError>
  where
    F: FnOnce(String, AnyMessage) -> Result<(), SchedulerError>, {
    let name = name.into();
    self.cancel_replaced_named_timer(ctx, &name);
    let generation = self.next_named_timer_generation();
    let timer_key = self.named_timer_key(&name);
    let fired = AnyMessage::new(FsmTimerFired::new(name.clone(), generation, message));
    schedule(timer_key.clone(), fired).map_err(|error| Self::scheduler_error_to_actor_error(&error))?;
    let previous = self.named_timers.insert(name, FsmNamedTimer::new(generation, is_repeating, timer_key));
    debug_assert!(previous.is_none());
    Ok(())
  }

  fn cancel_all_named_timers_best_effort(&mut self, ctx: &ActorContext<'_>) {
    for (name, timer) in self.named_timers.drain() {
      if let Err(error) = ctx.timers().cancel(timer.timer_key()) {
        Self::log_scheduler_warning(ctx, format!("fsm named timer stop cleanup failed for {name}: {error:?}"));
      }
    }
  }

  fn named_timer_key(&self, name: &str) -> String {
    format!("{}-{name}", self.named_timer_key_prefix)
  }

  // CQS exception: bump and read are inseparable in a single call, same pattern as Vec::pop
  const fn next_named_timer_generation(&mut self) -> u64 {
    self.named_timer_generation = self.named_timer_generation.wrapping_add(1);
    if self.named_timer_generation == 0 {
      self.named_timer_generation = 1;
    }
    self.named_timer_generation
  }

  fn log_scheduler_warning(ctx: &ActorContext<'_>, message: String) {
    ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
  }

  fn scheduler_error_to_actor_error(error: &SchedulerError) -> ActorError {
    ActorError::recoverable(ActorErrorReason::typed::<SchedulerError>(format!("fsm timer operation failed: {error:?}")))
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
