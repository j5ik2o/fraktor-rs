//! Event payload describing an unhandled message in the typed behavior system.

use alloc::string::String;
use core::time::Duration;

use crate::core::actor_prim::Pid;

/// Event emitted when a behavior returns `Behaviors.unhandled()`.
///
/// This event is published to the event stream for monitoring and debugging purposes.
/// Unlike `DeadLetter`, unhandled messages indicate that the actor is alive but chose
/// not to handle a particular message.
#[derive(Clone, Debug)]
pub struct UnhandledMessageEvent {
  /// The actor that did not handle the message.
  actor:     Pid,
  /// A description of the message type (e.g., "TypeName").
  message:   String,
  /// Timestamp when the event was created.
  timestamp: Duration,
}

impl UnhandledMessageEvent {
  /// Creates a new unhandled message event.
  #[must_use]
  pub const fn new(actor: Pid, message: String, timestamp: Duration) -> Self {
    Self { actor, message, timestamp }
  }

  /// Returns the actor pid that did not handle the message.
  #[must_use]
  pub const fn actor(&self) -> Pid {
    self.actor
  }

  /// Returns a description of the unhandled message.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn message(&self) -> &str {
    &self.message
  }

  /// Returns the timestamp associated with the event.
  #[must_use]
  pub const fn timestamp(&self) -> Duration {
    self.timestamp
  }
}
