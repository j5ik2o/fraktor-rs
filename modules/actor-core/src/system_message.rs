//! Internal system messages exchanged within the actor runtime.

use crate::{any_message::AnyMessage, RuntimeToolbox};

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

impl<TB: RuntimeToolbox> From<SystemMessage> for AnyMessage<TB> {
  fn from(value: SystemMessage) -> Self {
    AnyMessage::new(value)
  }
}
