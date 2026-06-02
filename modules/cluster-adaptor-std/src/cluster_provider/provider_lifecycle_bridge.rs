//! Provider lifecycle bridge for seed and discovery input.

use std::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  cluster_provider::{
    ClusterProvider, DiscoveryTopologyMapper, LocalClusterProviderShared, LocalClusterProviderWeak, SeedNodeInput,
    SeedNodeProcess,
  },
  extension::ClusterProviderError,
  topology::TopologyUpdate,
};
use fraktor_utils_core_rs::{sync::SharedAccess, time::TimerInstant};

use super::{DiscoveryBackend, GenericDiscoveryAdapter};

#[cfg(test)]
#[path = "provider_lifecycle_bridge_test.rs"]
mod tests;

/// Bridges provider start/shutdown with seed and discovery lifecycle.
pub struct ProviderLifecycleBridge<B> {
  provider:              LocalClusterProviderWeak,
  seed_process:          SeedNodeProcess,
  seed_input:            SeedNodeInput,
  discovery_adapter:     GenericDiscoveryAdapter<B>,
  topology_mapper:       DiscoveryTopologyMapper,
  next_observation_tick: u64,
  is_shutdown:           bool,
}

impl<B> ProviderLifecycleBridge<B> {
  /// Creates a provider lifecycle bridge.
  #[must_use]
  pub fn new(
    provider: LocalClusterProviderWeak,
    seed_input: SeedNodeInput,
    mut discovery_adapter: GenericDiscoveryAdapter<B>,
    topology_mapper: DiscoveryTopologyMapper,
  ) -> Self {
    discovery_adapter.attach_provider(provider.clone());
    Self {
      provider,
      seed_process: SeedNodeProcess::new(),
      seed_input,
      discovery_adapter,
      topology_mapper,
      next_observation_tick: 1,
      is_shutdown: false,
    }
  }

  /// Returns whether the bridge has been shut down.
  #[must_use]
  pub const fn is_shutdown(&self) -> bool {
    self.is_shutdown
  }

  /// Returns whether the weak provider handle can still be upgraded.
  #[must_use]
  pub fn provider_is_alive(&self) -> bool {
    self.provider.upgrade().is_some()
  }
}

impl<B> ProviderLifecycleBridge<B>
where
  B: DiscoveryBackend,
{
  /// Starts member lifecycle and applies seed/discovery join input.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when provider startup or join input application fails.
  pub fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    if self.is_shutdown {
      return Ok(());
    }

    let seed_joins = self.seed_process.start_member(&self.seed_input)?;
    let provider = self.provider.upgrade().ok_or_else(|| ClusterProviderError::start_member("provider unavailable"))?;
    provider.with_write(ClusterProvider::start_member)?;

    for authority in seed_joins {
      provider.with_write(|provider| provider.join(authority.as_str()))?;
    }

    self.refresh_discovery()
  }

  /// Starts client lifecycle without producing full member self-registration.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when provider client startup or seed validation fails.
  pub fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    if self.is_shutdown {
      return Ok(());
    }

    let provider = self.provider.upgrade().ok_or_else(|| ClusterProviderError::start_client("provider unavailable"))?;
    provider.with_write(ClusterProvider::start_client)?;
    let seed_joins = self.seed_process.start_client(&self.seed_input)?;
    debug_assert!(seed_joins.is_empty());
    Ok(())
  }

  /// Polls discovery and applies any joined/left topology delta.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when provider join/leave application fails.
  pub fn refresh_discovery(&mut self) -> Result<(), ClusterProviderError> {
    if self.is_shutdown {
      return Ok(());
    }

    let provider = self.provider.upgrade().ok_or_else(|| ClusterProviderError::start_member("provider unavailable"))?;
    let observed_at = self.next_observed_at();
    if let Some(result) = self.discovery_adapter.poll(observed_at) {
      if let Some(error) = result.error() {
        return Err(error.clone());
      }
      if let Some(update) = self.topology_mapper.apply(&result) {
        Self::apply_topology_update(&provider, &update)?;
      }
    }
    Ok(())
  }

  /// Shuts down provider, seed, and discovery lifecycle.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterProviderError`] when seed or provider shutdown fails.
  pub fn shutdown(&mut self, graceful: bool) -> Result<(), ClusterProviderError> {
    self.seed_process.shutdown()?;
    self.discovery_adapter.shutdown();
    self.is_shutdown = true;

    if let Some(provider) = self.provider.upgrade() {
      provider.with_write(|provider| provider.shutdown(graceful))?;
    }
    Ok(())
  }

  fn apply_topology_update(
    provider: &LocalClusterProviderShared,
    update: &TopologyUpdate,
  ) -> Result<(), ClusterProviderError> {
    for authority in update.joined.iter() {
      provider.with_write(|provider| provider.join(authority.as_str()))?;
    }
    for authority in update.left.iter() {
      provider.with_write(|provider| provider.leave(authority.as_str()))?;
    }
    Ok(())
  }

  fn next_observed_at(&mut self) -> TimerInstant {
    let observed_at = TimerInstant::from_ticks(self.next_observation_tick, Duration::from_secs(1));
    self.next_observation_tick = self.next_observation_tick.saturating_add(1);
    observed_at
  }
}
