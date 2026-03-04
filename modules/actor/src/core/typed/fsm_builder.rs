//! Minimal FSM DSL builder for typed behaviors.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::{fmt::Debug, hash::Hash};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
use hashbrown::HashMap;

use crate::core::typed::{Behaviors, behavior::Behavior};

type TransitionHandler<State, Message> = dyn Fn(&Message) -> Option<State> + Send + Sync;

/// Minimal FSM builder for composing state transition behaviors.
pub struct FsmBuilder<State, Message>
where
  State: Clone + Debug + Eq + Hash + Send + Sync + 'static,
  Message: Send + Sync + 'static, {
  initial_state: State,
  transitions:   Vec<(State, Box<TransitionHandler<State, Message>>)>,
}

impl<State, Message> FsmBuilder<State, Message>
where
  State: Clone + Debug + Eq + Hash + Send + Sync + 'static,
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
    self.transitions.push((state, Box::new(handler)));
    self
  }

  /// Builds a typed behavior that evaluates transitions on each message.
  ///
  /// # Panics
  ///
  /// Panics if duplicate transition handlers are registered for the same state.
  #[must_use]
  pub fn build(self) -> Behavior<Message> {
    let mut transition_map = HashMap::with_capacity_and_hasher(self.transitions.len(), RandomState::new());
    for (state, handler) in self.transitions {
      let prev = transition_map.insert(state.clone(), handler);
      assert!(prev.is_none(), "FsmBuilder: duplicate transition for state {:?}", state);
    }
    let transitions = ArcShared::new(transition_map);
    let state = ArcShared::new(RuntimeMutex::new(self.initial_state));

    Behaviors::receive_message(move |_ctx, message| {
      let current_state = state.lock().clone();
      let next_state = match transitions.get(&current_state) {
        | Some(handler) => (handler)(message),
        | None => return Ok(Behaviors::unhandled()),
      };
      if let Some(new_state) = next_state {
        *state.lock() = new_state;
      }
      Ok(Behaviors::same())
    })
  }
}
