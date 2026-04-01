//! Extension ID for [`SystemMaterializer`](crate::std::materializer::SystemMaterializer).

extern crate std;

use fraktor_actor_rs::core::kernel::{actor::extension::ExtensionId, system::ActorSystem};

use super::system_materializer::SystemMaterializer;
use crate::core::materialization::{ActorMaterializer, ActorMaterializerConfig};

/// Extension ID for [`SystemMaterializer`].
///
/// Use with [`ExtendedActorSystem::register_extension`] to register
/// a shared materializer, and [`ExtendedActorSystem::extension`] to retrieve it.
///
/// # Example
///
/// ```text
/// let ext = system.extended().register_extension(&SystemMaterializerId);
/// let materializer = ext.materializer();
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMaterializerId;

impl SystemMaterializerId {
  /// Creates a new extension ID instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl ExtensionId for SystemMaterializerId {
  type Ext = SystemMaterializer;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    SystemMaterializer::new(ActorMaterializer::new(system.clone(), ActorMaterializerConfig::new()))
  }
}
