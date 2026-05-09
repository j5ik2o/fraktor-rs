#[cfg(test)]
mod tests;

use super::StreamError;

/// Result of an IO operation, holding byte count and completion status.
///
/// Corresponds to Pekko's `IOResult(count: Long, status: Try[Done])`.
/// Used as the materialized value of file and stream IO stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IOResult {
  count:  u64,
  status: Result<(), StreamError>,
}

impl IOResult {
  /// Creates a successful IO result with the given byte count.
  #[must_use]
  pub const fn successful(count: u64) -> Self {
    Self { count, status: Ok(()) }
  }

  /// Creates a failed IO result with the given byte count and error.
  #[must_use]
  pub const fn failed(count: u64, error: StreamError) -> Self {
    Self { count, status: Err(error) }
  }

  /// Returns the number of bytes processed.
  #[must_use]
  pub const fn count(&self) -> u64 {
    self.count
  }

  /// Returns `true` if the IO operation completed successfully.
  #[must_use]
  pub const fn was_successful(&self) -> bool {
    self.status.is_ok()
  }

  /// Returns the error if the IO operation failed.
  #[must_use]
  pub fn error(&self) -> Option<&StreamError> {
    self.status.as_ref().err()
  }

  /// Returns a new `IOResult` with the given byte count.
  #[must_use]
  pub fn with_count(self, count: u64) -> Self {
    Self { count, status: self.status }
  }

  /// Returns a new `IOResult` with the given status.
  #[must_use]
  pub fn with_status(self, status: Result<(), StreamError>) -> Self {
    Self { count: self.count, status }
  }
}
