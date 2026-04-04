//! Installer for the coordinated shutdown extension.

extern crate std;

use fraktor_actor_rs::core::kernel::{
  actor::extension::{ExtensionInstaller, install_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};

use super::coordinated_shutdown_id::CoordinatedShutdownId;

/// Installs the coordinated shutdown extension during actor system bootstrap.
pub struct CoordinatedShutdownInstaller;

impl ExtensionInstaller for CoordinatedShutdownInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extension_id = CoordinatedShutdownId;
    install_extension_id(system, &extension_id);
    Ok(())
  }
}
