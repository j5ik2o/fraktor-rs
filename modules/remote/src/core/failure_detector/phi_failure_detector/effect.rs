//! Outcome emitted when the detector transitions an authority state.

use alloc::string::String;

/// Outcome emitted when the detector transitions an authority state.
#[derive(Debug, PartialEq)]
pub enum PhiFailureDetectorEffect {
  /// Authority exceeded the threshold and became suspect.
  Suspect {
    /// Authority identifier.
    authority: String,
    /// Phi value observed when the suspect event was emitted.
    phi:       f64,
  },
  /// Authority recovered after previously being suspect.
  Reachable {
    /// Authority identifier.
    authority: String,
  },
}
