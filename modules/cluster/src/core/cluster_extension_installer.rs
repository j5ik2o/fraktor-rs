//! Installs the cluster extension into an actor system.

use fraktor_actor_rs::core::{
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::cluster_extension_id::ClusterExtensionId;

/// Registers the cluster extension at actor system build time.
#[derive(Clone)]
pub struct ClusterExtensionInstaller<TB>
where
  TB: RuntimeToolbox + 'static, {
  id: ClusterExtensionId<TB>,
}

impl<TB> ClusterExtensionInstaller<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new installer that wraps the provided extension identifier.
  #[must_use]
  pub const fn new(id: ClusterExtensionId<TB>) -> Self {
    Self { id }
  }
}

impl<TB> ExtensionInstaller<TB> for ClusterExtensionInstaller<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let _ = system.extended().register_extension(&self.id);
    Ok(())
  }
}
