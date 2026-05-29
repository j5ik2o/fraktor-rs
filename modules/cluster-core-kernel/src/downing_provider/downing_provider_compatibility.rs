//! Downing provider compatibility metadata for join checks.

use alloc::string::String;

use super::SplitBrainResolverSettings;

pub(crate) const NOOP_DOWNING_PROVIDER_KEY: &str = "noop";
const EMPTY_DOWNING_PROVIDER_KEY_REASON: &str = "downing provider compatibility key must not be empty";

/// Compatibility identity advertised by a configured downing provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningProviderCompatibility {
  provider_key:                  String,
  split_brain_resolver_settings: Option<SplitBrainResolverSettings>,
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
    Self { provider_key, split_brain_resolver_settings: None }
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

  /// Returns optional Split Brain Resolver settings.
  #[must_use]
  pub const fn split_brain_resolver_settings(&self) -> Option<&SplitBrainResolverSettings> {
    self.split_brain_resolver_settings.as_ref()
  }

  /// Attaches Split Brain Resolver settings to this compatibility identity.
  #[must_use]
  pub const fn with_split_brain_resolver_settings(mut self, settings: SplitBrainResolverSettings) -> Self {
    self.split_brain_resolver_settings = Some(settings);
    self
  }
}
