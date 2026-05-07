//! Aggregates extension installers for the actor system builder.

use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::ExtensionInstaller;
use crate::core::kernel::system::{ActorSystem, ActorSystemBuildError};

/// Collection of extension installers to be registered with the actor system.
#[derive(Default)]
pub struct ExtensionInstallers {
  installers: Vec<ArcShared<dyn ExtensionInstaller>>,
}

impl ExtensionInstallers {
  /// Adds a new installer to be executed after the actor system boots.
  #[must_use]
  pub fn with_extension_installer<E>(mut self, installer: E) -> Self
  where
    E: ExtensionInstaller + 'static, {
    self.installers.push(ArcShared::new(installer));
    self
  }

  /// Adds a caller-retained shared installer handle.
  #[must_use]
  pub fn with_shared_extension_installer<E>(mut self, installer: ArcShared<E>) -> Self
  where
    E: ExtensionInstaller + 'static, {
    let installer: ArcShared<dyn ExtensionInstaller> = installer;
    self.installers.push(installer);
    self
  }

  pub(crate) fn install_all(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    for installer in &self.installers {
      installer.install(system)?;
    }
    Ok(())
  }
}
