//! Result of attempting to adapt an external message.

#[cfg(test)]
mod tests;

use crate::typed::message_adapter::AdapterFailure;

/// Enumerates the possible results of adapter execution.
#[derive(Debug, PartialEq, Eq)]
pub enum AdapterOutcome<M> {
  /// Adapter successfully produced a typed message.
  Converted(M),
  /// Adapter executed but reported a failure.
  Failure(AdapterFailure),
  /// Registry had no matching adapter for the payload type.
  NotFound,
}

impl<M> AdapterOutcome<M> {
  /// Maps the converted value using the provided function.
  pub fn map<U, F>(self, map: F) -> AdapterOutcome<U>
  where
    F: FnOnce(M) -> U, {
    match self {
      | AdapterOutcome::Converted(value) => AdapterOutcome::Converted(map(value)),
      | AdapterOutcome::Failure(failure) => AdapterOutcome::Failure(failure),
      | AdapterOutcome::NotFound => AdapterOutcome::NotFound,
    }
  }
}
