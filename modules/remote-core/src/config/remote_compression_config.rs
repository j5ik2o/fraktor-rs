//! Typed compression settings carried by `RemoteConfig`.

use core::{num::NonZeroUsize, time::Duration};

const DEFAULT_COMPRESSION_MAX: usize = 256;
const DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL: Duration = Duration::from_secs(60);

const fn default_compression_max() -> NonZeroUsize {
  // SAFETY: DEFAULT_COMPRESSION_MAX is a fixed positive literal.
  unsafe { NonZeroUsize::new_unchecked(DEFAULT_COMPRESSION_MAX) }
}

/// Compression table settings for actor refs and serializer manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteCompressionConfig {
  actor_ref_max:                    Option<NonZeroUsize>,
  actor_ref_advertisement_interval: Duration,
  manifest_max:                     Option<NonZeroUsize>,
  manifest_advertisement_interval:  Duration,
}

impl RemoteCompressionConfig {
  /// Creates compression settings with Pekko-compatible defaults.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      actor_ref_max:                    Some(default_compression_max()),
      actor_ref_advertisement_interval: DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL,
      manifest_max:                     Some(default_compression_max()),
      manifest_advertisement_interval:  DEFAULT_COMPRESSION_ADVERTISEMENT_INTERVAL,
    }
  }

  /// Returns a copy with the actor-ref compression limit changed.
  #[must_use]
  pub const fn with_actor_ref_max(mut self, max: Option<NonZeroUsize>) -> Self {
    self.actor_ref_max = max;
    self
  }

  /// Returns a copy with the actor-ref advertisement interval changed.
  ///
  /// # Panics
  ///
  /// Panics when `interval` is zero.
  #[must_use]
  pub const fn with_actor_ref_advertisement_interval(mut self, interval: Duration) -> Self {
    assert!(!interval.is_zero(), "actor-ref advertisement interval must be greater than zero");
    self.actor_ref_advertisement_interval = interval;
    self
  }

  /// Returns a copy with the manifest compression limit changed.
  #[must_use]
  pub const fn with_manifest_max(mut self, max: Option<NonZeroUsize>) -> Self {
    self.manifest_max = max;
    self
  }

  /// Returns a copy with the manifest advertisement interval changed.
  ///
  /// # Panics
  ///
  /// Panics when `interval` is zero.
  #[must_use]
  pub const fn with_manifest_advertisement_interval(mut self, interval: Duration) -> Self {
    assert!(!interval.is_zero(), "manifest advertisement interval must be greater than zero");
    self.manifest_advertisement_interval = interval;
    self
  }

  /// Returns the actor-ref compression limit.
  #[must_use]
  pub const fn actor_ref_max(&self) -> Option<NonZeroUsize> {
    self.actor_ref_max
  }

  /// Returns the actor-ref advertisement interval.
  #[must_use]
  pub const fn actor_ref_advertisement_interval(&self) -> Duration {
    self.actor_ref_advertisement_interval
  }

  /// Returns the manifest compression limit.
  #[must_use]
  pub const fn manifest_max(&self) -> Option<NonZeroUsize> {
    self.manifest_max
  }

  /// Returns the manifest advertisement interval.
  #[must_use]
  pub const fn manifest_advertisement_interval(&self) -> Duration {
    self.manifest_advertisement_interval
  }
}

impl Default for RemoteCompressionConfig {
  fn default() -> Self {
    Self::new()
  }
}
