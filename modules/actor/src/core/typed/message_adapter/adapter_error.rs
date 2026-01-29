//! Errors surfaced by message adapter infrastructure.

#[cfg(test)]
mod tests;

/// Represents configuration or runtime issues when handling adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterError {
  /// Registry reached the configured capacity.
  RegistryFull,
  /// Adapter envelope became inconsistent while transiting the runtime.
  EnvelopeCorrupted,
  /// The owning actor cell was not available.
  ActorUnavailable,
  /// Adapter registration attempted outside of a typed adapter context.
  RegistryUnavailable,
}
