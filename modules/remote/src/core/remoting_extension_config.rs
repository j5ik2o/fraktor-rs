//! Configuration applied when bootstrapping the remoting extension.

use alloc::{string::String, vec::Vec};
use core::{fmt, mem};

use fraktor_actor_rs::core::{
  config::RemotingConfig,
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::{
  RemotingBackpressureListener, RemotingExtensionId,
  core::failure_detector::phi_failure_detector_config::PhiFailureDetectorConfig,
};

/// Configures remoting bootstrap behaviour.
#[derive(Clone)]
pub struct RemotingExtensionConfig {
  remoting:               RemotingConfig,
  auto_start:             bool,
  transport_scheme:       String,
  backpressure_listeners: Vec<ArcShared<dyn RemotingBackpressureListener>>,
  failure_detector:       PhiFailureDetectorConfig,
}

impl RemotingExtensionConfig {
  /// Creates a new configuration from an existing remoting config.
  #[must_use]
  pub fn new(remoting: RemotingConfig) -> Self {
    Self {
      remoting,
      auto_start: true,
      transport_scheme: String::from("fraktor.loopback"),
      backpressure_listeners: Vec::new(),
      failure_detector: PhiFailureDetectorConfig::default(),
    }
  }

  /// Sets whether remoting should automatically start during actor system bootstrap.
  #[must_use]
  pub const fn with_auto_start(mut self, auto_start: bool) -> Self {
    self.auto_start = auto_start;
    self
  }

  /// Returns true when auto-start is enabled.
  #[must_use]
  pub const fn auto_start(&self) -> bool {
    self.auto_start
  }

  /// Returns the configured transport scheme.
  #[must_use]
  pub fn transport_scheme(&self) -> &str {
    &self.transport_scheme
  }

  /// Returns the underlying remoting configuration.
  #[must_use]
  pub const fn remoting(&self) -> &RemotingConfig {
    &self.remoting
  }

  /// Overrides the canonical host.
  #[must_use]
  pub fn with_canonical_host(mut self, host: impl Into<String>) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_canonical_host(host);
    self
  }

  /// Overrides the canonical port.
  #[must_use]
  pub fn with_canonical_port(mut self, port: u16) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_canonical_port(port);
    self
  }

  /// Overrides the quarantine duration.
  #[must_use]
  pub fn with_quarantine_duration(mut self, duration: core::time::Duration) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_quarantine_duration(duration);
    self
  }

  /// Overrides the transport scheme resolved by the factory.
  #[must_use]
  pub fn with_transport_scheme(mut self, scheme: impl Into<String>) -> Self {
    self.transport_scheme = scheme.into();
    self
  }

  /// Adds a backpressure listener that will be registered during bootstrap.
  #[must_use]
  pub fn with_backpressure_listener_arc(mut self, listener: ArcShared<dyn RemotingBackpressureListener>) -> Self {
    self.backpressure_listeners.push(listener);
    self
  }

  /// Adds a backpressure listener from a concrete type.
  #[must_use]
  pub fn with_backpressure_listener<L>(self, listener: L) -> Self
  where
    L: RemotingBackpressureListener, {
    self.with_backpressure_listener_arc(ArcShared::new(listener))
  }

  /// Returns the configured backpressure listeners.
  #[must_use]
  pub fn backpressure_listeners(&self) -> &[ArcShared<dyn RemotingBackpressureListener>] {
    &self.backpressure_listeners
  }

  /// Overrides the failure detector configuration.
  #[must_use]
  pub const fn with_failure_detector_config(mut self, config: PhiFailureDetectorConfig) -> Self {
    self.failure_detector = config;
    self
  }

  /// Returns the configured failure detector parameters.
  #[must_use]
  pub const fn failure_detector_config(&self) -> &PhiFailureDetectorConfig {
    &self.failure_detector
  }
}

impl Default for RemotingExtensionConfig {
  fn default() -> Self {
    Self::new(RemotingConfig::default())
  }
}

impl fmt::Debug for RemotingExtensionConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RemotingExtensionConfig")
      .field("remoting", &self.remoting)
      .field("auto_start", &self.auto_start)
      .field("transport_scheme", &self.transport_scheme)
      .field("backpressure_listener_count", &self.backpressure_listeners.len())
      .field("failure_detector_threshold", &self.failure_detector.threshold())
      .finish()
  }
}

impl<TB> ExtensionInstaller<TB> for RemotingExtensionConfig
where
  TB: RuntimeToolbox + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let id = RemotingExtensionId::new(self.clone());
    let _ = system.register_extension(&id);
    Ok(())
  }
}
