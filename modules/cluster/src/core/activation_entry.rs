//! Activation entry persisted by activation storage.

use alloc::string::String;

use crate::core::activation_record::ActivationRecord;

/// Represents a persisted activation entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationEntry {
  /// Owner authority.
  pub owner:       String,
  /// Activation record details.
  pub record:      ActivationRecord,
  /// Observation timestamp in seconds.
  pub observed_at: u64,
}
