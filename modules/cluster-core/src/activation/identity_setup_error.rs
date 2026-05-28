//! Errors occurring during identity lookup setup.

use alloc::string::String;

/// Represents failures initializing identity lookup for a cluster mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentitySetupError {
  /// Underlying provider returned a failure reason.
  Provider(String),
}
