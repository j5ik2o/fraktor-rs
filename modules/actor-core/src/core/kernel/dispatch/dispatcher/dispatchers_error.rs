//! Errors returned by the [`Dispatchers`](super::Dispatchers) registry.

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Errors returned by [`Dispatchers`](super::dispatchers::Dispatchers) operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchersError {
  /// The identifier is already registered.
  Duplicate(String),
  /// No configurator is registered for the identifier.
  Unknown(String),
}

impl Display for DispatchersError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Duplicate(id) => write!(f, "dispatcher id `{id}` is already registered"),
      | Self::Unknown(id) => write!(f, "no dispatcher registered for id `{id}`"),
    }
  }
}

impl core::error::Error for DispatchersError {}
