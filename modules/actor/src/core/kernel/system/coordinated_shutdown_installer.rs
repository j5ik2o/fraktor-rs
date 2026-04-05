//! Installer for the coordinated shutdown extension.

use super::coordinated_shutdown_id::CoordinatedShutdownId;
use crate::core::kernel::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};

/// Installs the coordinated shutdown extension during actor system bootstrap.
pub struct CoordinatedShutdownInstaller;

impl ExtensionInstaller for CoordinatedShutdownInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extension_id = CoordinatedShutdownId;
    install_extension_id(system, &extension_id);
    Ok(())
  }
}
