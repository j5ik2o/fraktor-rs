//! Errors that can occur during PID resolution.

use alloc::string::String;

/// Fatal errors raised by the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
  /// URI/Path does not match the expected protoactor-go scheme.
  InvalidFormat {
    /// Human-readable reason.
    reason: String,
  },
}
