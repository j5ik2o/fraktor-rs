//! Events emitted by failure detectors.

use alloc::string::String;

/// Failure detector outcome.
#[derive(Clone, Debug, PartialEq)]
pub enum FailureDetectorEvent {
  /// Authority is suspected of being unreachable.
  Suspect {
    /// Remote authority identifier.
    authority: String,
    /// Calculated phi value used to trigger the event.
    phi:       f64,
  },
  /// Authority is reachable again after a suspect period.
  Reachable {
    /// Remote authority identifier.
    authority: String,
  },
}
