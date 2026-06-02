//! Generic discovery backend adapter contract.

use std::string::ToString;

use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{DiscoveredAuthority, DiscoveryResult, LocalClusterProviderWeak},
  extension::ClusterProviderError,
};
use fraktor_utils_core_rs::time::TimerInstant;

use super::DiscoveryBackend;

#[cfg(test)]
#[path = "generic_discovery_adapter_test.rs"]
mod tests;

/// Adapter that normalizes generic backend output into discovery results.
pub struct GenericDiscoveryAdapter<B> {
  backend:     B,
  provider:    Option<LocalClusterProviderWeak>,
  is_shutdown: bool,
}

impl<B> GenericDiscoveryAdapter<B> {
  /// Creates a generic discovery adapter.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self { backend, provider: None, is_shutdown: false }
  }

  /// Attaches a weak provider handle for lifecycle boundary checks.
  pub fn attach_provider(&mut self, provider: LocalClusterProviderWeak) {
    self.provider = Some(provider);
  }

  /// Returns whether the attached provider is still alive.
  #[must_use]
  pub fn provider_is_alive(&self) -> bool {
    self.provider.as_ref().is_some_and(|provider| provider.upgrade().is_some())
  }

  /// Stops synchronous polling/subscription lifecycle.
  pub const fn shutdown(&mut self) {
    self.is_shutdown = true;
  }

  /// Returns whether discovery lifecycle has been stopped.
  #[must_use]
  pub const fn is_shutdown(&self) -> bool {
    self.is_shutdown
  }
}

impl<B> GenericDiscoveryAdapter<B>
where
  B: DiscoveryBackend,
{
  /// Polls the backend when lifecycle is still active.
  #[must_use]
  pub fn poll(&mut self, observed_at: TimerInstant) -> Option<DiscoveryResult> {
    if self.is_shutdown || self.provider.as_ref().is_some_and(|provider| provider.upgrade().is_none()) {
      return None;
    }
    Some(self.discover(observed_at))
  }

  /// Runs the backend and returns a provider-neutral discovery result.
  #[must_use]
  pub fn discover(&mut self, observed_at: TimerInstant) -> DiscoveryResult {
    let source_identity = self.backend.source_identity().to_string();
    match self.backend.discover() {
      | Ok(authorities) if authorities.is_empty() => DiscoveryResult::empty(source_identity, observed_at),
      | Ok(authorities) if authorities.iter().any(|authority| !Self::is_valid_authority(authority)) => {
        DiscoveryResult::failed(source_identity, observed_at, ClusterProviderError::join("invalid discovery authority"))
      },
      | Ok(authorities) => DiscoveryResult::discovered(
        authorities
          .into_iter()
          .map(|authority| DiscoveredAuthority::new(authority, source_identity.clone(), observed_at))
          .collect(),
      ),
      | Err(error) => DiscoveryResult::failed(source_identity, observed_at, error.into()),
    }
  }

  fn is_valid_authority(authority: &str) -> bool {
    !authority.is_empty() && !authority.chars().any(char::is_whitespace)
  }
}
