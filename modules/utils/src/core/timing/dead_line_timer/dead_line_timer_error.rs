//! Error definitions associated with deadline timer operations.

#[cfg(test)]
mod tests;

/// Errors produced by DeadlineTimer implementations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DeadLineTimerError {
  /// The timer is full and cannot accept more entries.
  #[error("deadline timer is full")]
  Full,
  /// The provided key was not found.
  #[error("key not found")]
  NotFound,
  /// The timer backend failed.
  #[error("backend failure")]
  BackendFailure,
}
