//! Minimal FSM DSL builder for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeMutex, sync::ArcShared};

use crate::core::typed::{Behaviors, behavior::Behavior};

type TransitionHandler<State, Message> = dyn Fn(&Message) -> Option<State> + Send + Sync;

struct FsmTransition<State, Message>
where
  State: Clone + PartialEq + Send + Sync + 'static,
  Message: Send + Sync + 'static, {
  state:   State,
  handler: Box<TransitionHandler<State, Message>>,
}

struct FsmRuntimeState<State, Message>
where
  State: Clone + PartialEq + Send + Sync + 'static,
  Message: Send + Sync + 'static, {
  state:       State,
  transitions: Vec<FsmTransition<State, Message>>,
}

/// Minimal FSM builder for composing state transition behaviors.
pub struct FsmBuilder<State, Message>
where
  State: Clone + PartialEq + Send + Sync + 'static,
  Message: Send + Sync + 'static, {
  initial_state: State,
  transitions:   Vec<FsmTransition<State, Message>>,
}

impl<State, Message> FsmBuilder<State, Message>
where
  State: Clone + PartialEq + Send + Sync + 'static,
  Message: Send + Sync + 'static,
{
  /// Creates a new FSM builder with the provided initial state.
  #[must_use]
  pub const fn new(initial_state: State) -> Self {
    Self { initial_state, transitions: Vec::new() }
  }

  /// Registers a transition handler for the specified state.
  ///
  /// The handler returns `Some(next_state)` when a transition should occur.
  /// Returning `None` keeps the current state.
  #[must_use]
  pub fn when<F>(mut self, state: State, handler: F) -> Self
  where
    F: Fn(&Message) -> Option<State> + Send + Sync + 'static, {
    self.transitions.push(FsmTransition { state, handler: Box::new(handler) });
    self
  }

  /// Builds a typed behavior that evaluates transitions on each message.
  #[must_use]
  pub fn build(self) -> Behavior<Message> {
    let runtime_state = ArcShared::new(RuntimeMutex::new(FsmRuntimeState {
      state:       self.initial_state,
      transitions: self.transitions,
    }));

    Behaviors::receive_message(move |_ctx, message| {
      let mut guard = runtime_state.lock();
      let current_state = guard.state.clone();
      let mut handler_found = false;
      let mut next_state: Option<State> = None;
      for transition in &guard.transitions {
        if transition.state == current_state {
          handler_found = true;
          next_state = (transition.handler)(message);
          break;
        }
      }
      if let Some(state) = next_state {
        guard.state = state;
        return Ok(Behaviors::same());
      }
      if handler_found {
        return Ok(Behaviors::same());
      }
      Ok(Behaviors::unhandled())
    })
  }
}
