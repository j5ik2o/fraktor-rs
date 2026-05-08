//! Pekko-compatible death pact exception for typed actors.

#[cfg(test)]
mod tests;

use core::fmt::{Display, Formatter, Result as FmtResult};

use fraktor_actor_core_rs::core::kernel::actor::Pid;

/// Exception thrown when a watched actor terminates and the watcher
/// does not handle the [`BehaviorSignal::Terminated`] signal.
///
/// This corresponds to Pekko's `DeathPactError(ref)` in
/// `actor-typed/MessageAndSignals.scala`.  The exception carries the
/// [`Pid`] of the terminated actor so that supervision strategies can
/// inspect which death pact was triggered.
///
/// [`BehaviorSignal::Terminated`]: crate::message_and_signals::BehaviorSignal::Terminated
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeathPactError {
  /// The terminated actor that triggered the death pact.
  terminated: Pid,
}

impl DeathPactError {
  /// Creates a new death pact exception for the given terminated actor.
  #[must_use]
  pub const fn new(terminated: Pid) -> Self {
    Self { terminated }
  }

  /// Returns the [`Pid`] of the terminated actor.
  #[must_use]
  pub const fn terminated(&self) -> Pid {
    self.terminated
  }
}

impl Display for DeathPactError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "death pact with {} was triggered", self.terminated)
  }
}
