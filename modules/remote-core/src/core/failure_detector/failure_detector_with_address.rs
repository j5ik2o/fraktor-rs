//! Address-aware failure detector extension.

use alloc::string::String;

/// Extension for detectors that receive their monitored address after creation.
pub trait FailureDetectorWithAddress {
  /// Sets the monitored address.
  fn set_address(&mut self, address: String);
}
