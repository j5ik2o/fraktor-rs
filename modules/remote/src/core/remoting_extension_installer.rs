//! Installer for the remoting extension.

use fraktor_actor_rs::core::{
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{remoting_extension_config::RemotingExtensionConfig, remoting_extension_id::RemotingExtensionId};

/// Installs the remoting extension into the actor system.
pub struct RemotingExtensionInstaller {
  config: RemotingExtensionConfig,
}

impl RemotingExtensionInstaller {
  /// Creates a new remoting extension installer with the specified configuration.
  pub fn new(config: RemotingExtensionConfig) -> Self {
    Self { config }
  }
}

impl<TB> ExtensionInstaller<TB> for RemotingExtensionInstaller
where
  TB: RuntimeToolbox + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
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

    let id = RemotingExtensionId::<TB>::new(merged_config);
    let _ = system.extended().register_extension(&id);
    Ok(())
  }
}
