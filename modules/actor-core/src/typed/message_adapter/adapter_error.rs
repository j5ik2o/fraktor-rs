//! Errors surfaced by message adapter infrastructure.

#[cfg(test)]
mod tests;

use core::any::TypeId;

/// Represents configuration or runtime issues when handling adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterError {
  /// Registry reached the configured capacity.
  RegistryFull,
  /// Adapter payload type mismatched expectations.
  TypeMismatch(TypeId),
  /// Adapter envelope became inconsistent while transiting the runtime.
  EnvelopeCorrupted,
  /// The owning actor cell was not available.
  ActorUnavailable,
  /// Adapter registration attempted outside of a typed adapter context.
  RegistryUnavailable,
}
