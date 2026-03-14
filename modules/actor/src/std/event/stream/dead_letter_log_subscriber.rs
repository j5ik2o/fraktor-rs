//! Event stream subscriber that logs dead letter events via `tracing`.

extern crate std;

#[cfg(test)]
mod tests;

use tracing::{Level, event};

use crate::{core::event::stream::EventStreamEvent, std::event::stream::EventStreamSubscriber};

/// Default target name used in emitted dead letter events.
const DEAD_LETTER_TARGET: &str = "fraktor::event::stream::dead_letter";

/// Event stream subscriber that logs every dead letter event.
///
/// Unlike Pekko's `DeadLetterListener` which is implemented as a classic actor,
/// this is a lightweight `EventStreamSubscriber` adapter suitable for fraktor-rs's
/// event stream architecture.
pub struct DeadLetterLogSubscriber {
  _private: (),
}

impl DeadLetterLogSubscriber {
  /// Creates a new subscriber.
  #[must_use]
  pub const fn new() -> Self {
    Self { _private: () }
  }
}

impl Default for DeadLetterLogSubscriber {
  fn default() -> Self {
    Self::new()
  }
}

impl EventStreamSubscriber for DeadLetterLogSubscriber {
  fn on_event(&mut self, stream_event: &EventStreamEvent) {
    if let EventStreamEvent::DeadLetter(entry) = stream_event {
      let recipient =
        entry.recipient().map(|pid| alloc::format!("{}", pid)).unwrap_or_else(|| alloc::string::String::from("n/a"));
      let reason = alloc::format!("{:?}", entry.reason());
      event!(
        target: DEAD_LETTER_TARGET,
        Level::WARN,
        recipient = recipient.as_str(),
        reason = reason.as_str(),
        "dead letter received"
      );
    }
  }
}
