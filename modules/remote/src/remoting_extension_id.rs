//! Extension identifier wiring remoting into the actor system.

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::{RemotingExtension, RemotingExtensionConfig};

/// Extension identifier storing the remoting configuration.
#[derive(Clone, Debug)]
pub struct RemotingExtensionId {
  config: RemotingExtensionConfig,
}

impl RemotingExtensionId {
  /// Creates a new identifier referencing the provided configuration.
  #[must_use]
  pub const fn new(config: RemotingExtensionConfig) -> Self {
    Self { config }
  }
}

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for RemotingExtensionId {
  type Ext = RemotingExtension<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
    RemotingExtension::new(system, self.config.clone())
  }
}
