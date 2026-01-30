//! Result of attempting to adapt an external message.

use crate::core::typed::message_adapter::AdapterError;

/// Enumerates the possible results of adapter execution.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdapterOutcome<M> {
  /// Adapter successfully produced a typed message.
  Converted(M),
  /// Adapter executed but reported a failure.
  Failure(AdapterError),
  /// Registry had no matching adapter for the payload type.
  NotFound,
}
