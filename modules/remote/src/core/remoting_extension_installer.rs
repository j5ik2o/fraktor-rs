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
    let id = RemotingExtensionId::<TB>::new(self.config.clone());
    let _ = system.extended().register_extension(&id);
    Ok(())
  }
}
