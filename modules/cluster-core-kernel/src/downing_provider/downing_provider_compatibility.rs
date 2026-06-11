//! Downing provider compatibility metadata for join checks.

use alloc::{format, string::String};

use super::{SplitBrainResolverConfig, SplitBrainResolverStrategy};

pub(crate) const NOOP_DOWNING_PROVIDER_KEY: &str = "noop";
const EMPTY_DOWNING_PROVIDER_KEY_REASON: &str = "downing provider compatibility key must not be empty";

/// Compatibility identity advertised by a configured downing provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningProviderCompatibility {
  provider_key:                String,
  split_brain_resolver_config: Option<SplitBrainResolverConfig>,
  sbr_config_identity:         Option<String>,
}

impl DowningProviderCompatibility {
  /// Creates compatibility metadata for the given provider key.
  ///
  /// # Panics
  ///
  /// Panics when `provider_key` is empty.
  #[must_use]
  pub fn new(provider_key: impl Into<String>) -> Self {
    let provider_key = provider_key.into();
    assert!(!provider_key.is_empty(), "{EMPTY_DOWNING_PROVIDER_KEY_REASON}");
    Self { provider_key, split_brain_resolver_config: None, sbr_config_identity: None }
  }

  /// Creates compatibility metadata for the built-in no-op downing provider.
  #[must_use]
  pub(crate) fn noop() -> Self {
    Self::new(NOOP_DOWNING_PROVIDER_KEY)
  }

  /// Returns the stable provider key used for compatibility comparison.
  #[must_use]
  pub fn provider_key(&self) -> &str {
    &self.provider_key
  }

  /// Returns optional Split Brain Resolver configuration.
  #[must_use]
  pub const fn split_brain_resolver_config(&self) -> Option<&SplitBrainResolverConfig> {
    self.split_brain_resolver_config.as_ref()
  }

  /// Returns the deterministic Split Brain Resolver configuration identity.
  #[must_use]
  pub fn sbr_config_identity(&self) -> Option<&str> {
    self.sbr_config_identity.as_deref()
  }

  /// Attaches Split Brain Resolver configuration to this compatibility identity.
  #[must_use]
  pub fn with_split_brain_resolver_config(mut self, config: SplitBrainResolverConfig) -> Self {
    self.split_brain_resolver_config = Some(config);
    self.sbr_config_identity = Some(format_sbr_config_identity(config));
    self
  }
}

fn format_sbr_config_identity(config: SplitBrainResolverConfig) -> String {
  let mut identity = format!(
    "stable-after-nanos={};active-strategy={};down-all-when-unstable-nanos={}",
    config.stable_after().as_nanos(),
    config.active_strategy().as_str(),
    config.down_all_when_unstable().as_nanos()
  );
  if let (SplitBrainResolverStrategy::StaticQuorum, Some(size)) =
    (config.active_strategy(), config.static_quorum_size())
  {
    identity.push_str(";static-quorum-size=");
    identity.push_str(&format!("{size}"));
  }
  identity
}
