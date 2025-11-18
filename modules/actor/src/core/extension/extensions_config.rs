//! Aggregates extension installers for the actor system builder.

use alloc::vec::Vec;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use super::ExtensionInstaller;
use crate::core::system::{ActorSystemBuildError, ActorSystemGeneric};

/// Builder-side configuration for registering extensions automatically.
pub struct ExtensionsConfig<TB>
where
  TB: RuntimeToolbox + 'static, {
  installers: Vec<ArcShared<dyn ExtensionInstaller<TB>>>,
}

impl<TB> ExtensionsConfig<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Adds a new installer to be executed after the actor system boots.
  #[must_use]
  pub fn with_extension_config<E>(mut self, installer: E) -> Self
  where
    E: ExtensionInstaller<TB> + 'static, {
    self.installers.push(ArcShared::new(installer));
    self
  }

  pub(crate) fn install_all(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    for installer in &self.installers {
      installer.install(system)?;
    }
    Ok(())
  }
}

impl<TB> Default for ExtensionsConfig<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self { installers: Vec::new() }
  }
}
