//! Represents failures occurring while executing adapter closures.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

/// Detailed failure captured during message adaptation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterFailure {
  /// The payload type did not match the registered adapter.
  TypeMismatch(TypeId),
  /// The adapter reported a domain-specific reason.
  Custom(String),
}
