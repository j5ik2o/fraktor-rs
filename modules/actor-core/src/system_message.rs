//! Internal system messages exchanged within the actor runtime.

use crate::any_message::AnyMessage;

/// Lightweight enum describing system-level mailbox traffic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemMessage {
  /// Requests the mailbox to suspend user message processing.
  Suspend,
  /// Requests the mailbox to resume user message processing.
  Resume,
  /// Signals that the associated actor should stop.
  Stop,
}

impl From<SystemMessage> for AnyMessage {
  fn from(value: SystemMessage) -> Self {
    AnyMessage::new(value)
  }
}
