//! Signal delivered when a message adapter cannot translate an external message.

use crate::{
  message_adapter::AdapterError,
  message_and_signals::{BehaviorSignal, Signal},
};

/// Public signal emitted when message adaption fails before behavior dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageAdaptionFailure {
  error: AdapterError,
}

impl MessageAdaptionFailure {
  /// Creates a new message-adaption-failure signal payload.
  #[must_use]
  pub const fn new(error: AdapterError) -> Self {
    Self { error }
  }

  /// Returns the adapter error that prevented message delivery.
  #[must_use]
  pub const fn error(&self) -> &AdapterError {
    &self.error
  }
}

impl Signal for MessageAdaptionFailure {}

impl From<MessageAdaptionFailure> for BehaviorSignal {
  fn from(value: MessageAdaptionFailure) -> Self {
    Self::MessageAdaptionFailure(value)
  }
}
