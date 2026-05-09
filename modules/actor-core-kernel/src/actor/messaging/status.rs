//! Classic status reply messages.

#[cfg(test)]
mod tests;

use crate::actor::{error::ActorError, messaging::AnyMessage};

/// Classic status reply envelope.
#[derive(Clone, Debug)]
pub enum Status {
  /// Successful reply payload.
  Success(AnyMessage),
  /// Failed reply error.
  Failure(ActorError),
}

impl Status {
  /// Creates a success status.
  #[must_use]
  pub const fn success(payload: AnyMessage) -> Self {
    Self::Success(payload)
  }

  /// Creates a failure status.
  #[must_use]
  pub const fn failure(error: ActorError) -> Self {
    Self::Failure(error)
  }
}
