use fraktor_utils_rs::core::sync::ArcShared;

use super::{dispatcher_provider::DispatcherProvider, dispatcher_settings::DispatcherSettings};

/// Dispatcher registry entry bundling provider and immutable settings.
pub struct DispatcherRegistryEntry {
  provider: ArcShared<dyn DispatcherProvider>,
  settings: DispatcherSettings,
}

impl Clone for DispatcherRegistryEntry {
  fn clone(&self) -> Self {
    Self { provider: self.provider.clone(), settings: self.settings.clone() }
  }
}

impl DispatcherRegistryEntry {
  /// Creates a registry entry from a provider and immutable settings snapshot.
  #[must_use]
  pub fn new<P>(provider: P, settings: DispatcherSettings) -> Self
  where
    P: DispatcherProvider + 'static, {
    let provider: ArcShared<dyn DispatcherProvider> = ArcShared::new(provider);
    Self { provider, settings }
  }

  /// Returns the provider.
  #[must_use]
  pub fn provider(&self) -> &ArcShared<dyn DispatcherProvider> {
    &self.provider
  }

  /// Returns the immutable settings snapshot.
  #[must_use]
  pub const fn settings(&self) -> &DispatcherSettings {
    &self.settings
  }
}
