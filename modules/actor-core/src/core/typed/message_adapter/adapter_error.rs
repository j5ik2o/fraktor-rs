//! Errors surfaced by message adapter infrastructure and adapter execution.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

/// Represents configuration or execution issues when handling adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterError {
  /// Registry reached the configured capacity.
  RegistryFull,
  /// Adapter envelope became inconsistent while transiting the runtime.
  EnvelopeCorrupted,
  /// The owning actor cell was not available.
  ActorUnavailable,
  /// Adapter registration attempted outside of a typed adapter context.
  RegistryUnavailable,
  /// The payload type did not match the registered adapter.
  TypeMismatch(TypeId),
  /// The adapter reported a domain-specific reason.
  Custom(String),
}
