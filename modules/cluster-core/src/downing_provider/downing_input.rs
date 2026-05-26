//! Input accepted by a downing strategy.

use alloc::string::String;

use super::FailureObservation;

/// Input passed into the core downing boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DowningInput {
  /// Explicit down command requested by a caller.
  ExplicitDown {
    /// Target authority.
    authority: String,
  },
  /// Availability observation from membership or failure detection.
  FailureObservation(FailureObservation),
}

impl DowningInput {
  /// Creates an explicit down input.
  #[must_use]
  pub fn explicit_down(authority: &str) -> Self {
    Self::ExplicitDown { authority: String::from(authority) }
  }
}
