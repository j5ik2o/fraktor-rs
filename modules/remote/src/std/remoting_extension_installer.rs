//! Installer for the remoting extension.

use fraktor_actor_rs::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};
use fraktor_utils_rs::core::sync::ArcShared;

use super::remoting_extension_id::RemotingExtensionId;
use crate::core::remoting_extension::RemotingExtensionConfig;

/// Installs the remoting extension into the actor system.
pub struct RemotingExtensionInstaller {
  config: RemotingExtensionConfig,
}

impl RemotingExtensionInstaller {
  /// Creates a new remoting extension installer with the specified configuration.
  #[must_use]
  pub fn new(config: RemotingExtensionConfig) -> Self {
    Self { config }
  }
}

impl ExtensionInstaller for RemotingExtensionInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    // システムの RemotingConfig から canonical_host/port を取得して、
    // 拡張設定で未設定の場合にマージする
    let mut merged_config = self.config.clone();

    if let Some(system_remoting) = system.remoting_config() {
      // canonical_host が空の場合、システムから取得
      if merged_config.canonical_host().is_empty() {
        merged_config = merged_config.with_canonical_host(system_remoting.canonical_host());
      }

      // canonical_port が未設定の場合、システムから取得
      if merged_config.canonical_port().is_none()
        && let Some(port) = system_remoting.canonical_port()
      {
        merged_config = merged_config.with_canonical_port(port);
      }
    }

    let id = RemotingExtensionId::new(merged_config);
    let registered = system.extended().register_extension(&id);
    let existing = system
      .extended()
      .extension(&id)
      .ok_or_else(|| ActorSystemBuildError::Configuration("remoting extension was not retained".into()))?;
    if !ArcShared::ptr_eq(&registered, &existing) {
      return Err(ActorSystemBuildError::Configuration("remoting extension identity mismatch".into()));
    }
    Ok(())
  }
}
