//! Transition directives returned by classic FSM handlers.

use alloc::vec::Vec;
use core::time::Duration;

use super::FsmReason;
use crate::core::kernel::actor::messaging::AnyMessage;

/// Transition directive returned from a classic FSM state handler.
pub struct FsmTransition<State, Data> {
  next_state:      Option<State>,
  next_data:       Option<Data>,
  stop_reason:     Option<FsmReason>,
  for_max_timeout: Option<Option<Duration>>,
  replies:         Vec<AnyMessage>,
  handled:         bool,
}

impl<State, Data> FsmTransition<State, Data> {
  /// Keeps the current state and data.
  #[must_use]
  pub const fn stay() -> Self {
    Self {
      next_state:      None,
      next_data:       None,
      stop_reason:     None,
      for_max_timeout: None,
      replies:         Vec::new(),
      handled:         true,
    }
  }

  /// Moves to the provided next state.
  #[must_use]
  pub const fn goto(next_state: State) -> Self {
    Self {
      next_state:      Some(next_state),
      next_data:       None,
      stop_reason:     None,
      for_max_timeout: None,
      replies:         Vec::new(),
      handled:         true,
    }
  }

  /// Stops the FSM with the provided reason.
  #[must_use]
  pub const fn stop(reason: FsmReason) -> Self {
    Self {
      next_state:      None,
      next_data:       None,
      stop_reason:     Some(reason),
      for_max_timeout: None,
      replies:         Vec::new(),
      handled:         true,
    }
  }

  /// Marks the message as not handled by the current FSM state.
  #[must_use]
  pub const fn unhandled() -> Self {
    Self {
      next_state:      None,
      next_data:       None,
      stop_reason:     None,
      for_max_timeout: None,
      replies:         Vec::new(),
      handled:         false,
    }
  }

  /// Replaces the state data associated with the next state.
  #[must_use]
  pub fn using(mut self, data: Data) -> Self {
    self.next_data = Some(data);
    self
  }

  /// Overrides the current transition's state timeout once, mirroring Pekko `forMax`.
  ///
  /// `Some(duration)` installs a transient timeout for the resulting state and
  /// leaves the `state_timeouts` registration unchanged. `None` cancels the
  /// currently armed timeout for this transition only. `Duration::ZERO` is
  /// normalized to the same cancel behaviour as `None`.
  #[must_use]
  pub fn for_max(mut self, timeout: Option<Duration>) -> Self {
    self.for_max_timeout = Some(timeout.filter(|duration| !duration.is_zero()));
    self
  }

  /// Queues a reply to the current sender, mirroring Pekko `replying`.
  ///
  /// Multiple calls preserve call order when replies are dispatched.
  #[must_use]
  pub fn replying(mut self, reply: AnyMessage) -> Self {
    self.replies.push(reply);
    self
  }

  pub(crate) const fn handled(&self) -> bool {
    self.handled
  }

  pub(crate) const fn for_max_timeout(&self) -> Option<Option<Duration>> {
    self.for_max_timeout
  }

  pub(crate) fn take_replies(&mut self) -> Vec<AnyMessage> {
    core::mem::take(&mut self.replies)
  }

  pub(crate) fn into_parts(self) -> (Option<State>, Option<Data>, Option<FsmReason>) {
    (self.next_state, self.next_data, self.stop_reason)
  }
}
