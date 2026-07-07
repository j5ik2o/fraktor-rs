//! Explicit opt-in installer for Cluster Singleton manager/proxy runtime.

#[cfg(test)]
#[path = "singleton_extension_installer_test.rs"]
mod tests;

use alloc::{format, string::String};

use fraktor_actor_core_kernel_rs::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_cluster_core_kernel_rs::{
  extension::ClusterExtension,
  singleton::{ClusterSingletonConfigError, ClusterSingletonManagerConfig, ClusterSingletonProxyConfig},
};

/// Installs Cluster Singleton manager/proxy runtime as an explicit opt-in extension.
pub struct ClusterSingletonExtensionInstaller {
  manager_config: ClusterSingletonManagerConfig,
  proxy_config:   ClusterSingletonProxyConfig,
}

impl ClusterSingletonExtensionInstaller {
  /// Creates a new installer with manager and proxy configuration.
  #[must_use]
  pub const fn new(manager_config: ClusterSingletonManagerConfig, proxy_config: ClusterSingletonProxyConfig) -> Self {
    Self { manager_config, proxy_config }
  }

  /// Returns the manager configuration.
  #[must_use]
  pub const fn manager_config(&self) -> &ClusterSingletonManagerConfig {
    &self.manager_config
  }

  /// Returns the proxy configuration.
  #[must_use]
  pub const fn proxy_config(&self) -> &ClusterSingletonProxyConfig {
    &self.proxy_config
  }
}

impl ExtensionInstaller for ClusterSingletonExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.manager_config.validate().map_err(map_singleton_error)?;
    self.proxy_config.validate().map_err(map_singleton_error)?;
    system
      .extended()
      .extension_by_type::<ClusterExtension>()
      .ok_or_else(|| ActorSystemBuildError::Configuration(String::from("cluster extension not installed")))?;
    Ok(())
  }
}

fn map_singleton_error(error: ClusterSingletonConfigError) -> ActorSystemBuildError {
  ActorSystemBuildError::Configuration(format!("{error:?}"))
}
