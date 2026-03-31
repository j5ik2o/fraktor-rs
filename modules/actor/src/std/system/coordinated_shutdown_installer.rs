//! Installer for the coordinated shutdown extension.

extern crate std;

use alloc::format;

use super::coordinated_shutdown_id::CoordinatedShutdownId;
use crate::core::kernel::{
  actor::extension::ExtensionInstaller,
  system::{ActorSystem, ActorSystemBuildError},
};

/// Installs the coordinated shutdown extension during actor system bootstrap.
pub struct CoordinatedShutdownInstaller;

impl ExtensionInstaller for CoordinatedShutdownInstaller {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    let extension_id = CoordinatedShutdownId;
    system.extended().register_extension(&extension_id).map(|_| ()).map_err(|error| {
      ActorSystemBuildError::Configuration(format!("coordinated shutdown extension registration failed: {error:?}"))
    })
  }
}
