//! Input accepted by a downing strategy.

use alloc::string::String;

use super::FailureObservation;
use crate::membership::IndirectConnectionEvidence;

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
  /// Indirect connectivity evidence from the membership reachability matrix.
  IndirectConnectionEvidence(IndirectConnectionEvidence),
}

impl DowningInput {
  /// Creates an explicit down input.
  #[must_use]
  pub fn explicit_down(authority: &str) -> Self {
    Self::ExplicitDown { authority: String::from(authority) }
  }
}
