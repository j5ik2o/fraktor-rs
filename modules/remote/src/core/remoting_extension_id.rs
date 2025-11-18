//! Extension identifier bridging the actor system registry and the remoting implementation.

use core::marker::PhantomData;

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{remoting_extension::RemotingExtension, remoting_extension_config::RemotingExtensionConfig};

/// Registers and instantiates [`RemotingExtension`] instances.
pub struct RemotingExtensionId<TB>
where
  TB: RuntimeToolbox + 'static, {
  config: RemotingExtensionConfig,
  marker: PhantomData<TB>,
}

impl<TB> RemotingExtensionId<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new identifier with the provided configuration.
  #[must_use]
  pub fn new(config: RemotingExtensionConfig) -> Self {
    Self { config, marker: PhantomData }
  }
}

impl<TB> ExtensionId<TB> for RemotingExtensionId<TB>
where
  TB: RuntimeToolbox + 'static,
{
  type Ext = RemotingExtension<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
    RemotingExtension::new(system, &self.config)
  }
}
