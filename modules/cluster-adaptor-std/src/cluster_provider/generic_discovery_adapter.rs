//! Generic discovery backend adapter contract.

use std::string::ToString;

use fraktor_cluster_core_kernel_rs::cluster_provider::{DiscoveredAuthority, DiscoveryResult};
use fraktor_utils_core_rs::time::TimerInstant;

use super::DiscoveryBackend;

#[cfg(test)]
#[path = "generic_discovery_adapter_test.rs"]
mod tests;

/// Adapter that normalizes generic backend output into discovery results.
pub struct GenericDiscoveryAdapter<B> {
  backend: B,
}

impl<B> GenericDiscoveryAdapter<B> {
  /// Creates a generic discovery adapter.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self { backend }
  }
}

impl<B> GenericDiscoveryAdapter<B>
where
  B: DiscoveryBackend,
{
  /// Runs the backend and returns a provider-neutral discovery result.
  #[must_use]
  pub fn discover(&mut self, observed_at: TimerInstant) -> DiscoveryResult {
    let source_identity = self.backend.source_identity().to_string();
    match self.backend.discover() {
      | Ok(authorities) if authorities.is_empty() => DiscoveryResult::empty(source_identity, observed_at),
      | Ok(authorities) => DiscoveryResult::discovered(
        authorities
          .into_iter()
          .map(|authority| DiscoveredAuthority::new(authority, source_identity.clone(), observed_at))
          .collect(),
      ),
      | Err(error) => DiscoveryResult::failed(source_identity, observed_at, error.into()),
    }
  }
}
