//! Installer for the coordinated shutdown extension.

extern crate std;

use fraktor_utils_rs::core::sync::ArcShared;

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
    let registered = system.extended().register_extension(&extension_id);
    let existing = system
      .extended()
      .extension(&extension_id)
      .ok_or_else(|| ActorSystemBuildError::Configuration("coordinated shutdown extension was not retained".into()))?;
    if !ArcShared::ptr_eq(&registered, &existing) {
      return Err(ActorSystemBuildError::Configuration("coordinated shutdown extension identity mismatch".into()));
    }
    Ok(())
  }
}
