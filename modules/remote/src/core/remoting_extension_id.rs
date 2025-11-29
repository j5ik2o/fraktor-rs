//! Extension identifier bridging the actor system registry and the remoting implementation.

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::{RemotingExtensionGeneric, remoting_extension_config::RemotingExtensionConfig};

/// Registers and instantiates [`RemotingExtension`] instances.
///
/// This type is only available with the `std` feature because the extension
/// initialization requires `TransportFactory` which depends on standard library facilities.
pub struct RemotingExtensionId {
  config: RemotingExtensionConfig,
}

impl RemotingExtensionId {
  /// Creates a new identifier with the provided configuration.
  #[must_use]
  pub fn new(config: RemotingExtensionConfig) -> Self {
    Self { config }
  }
}

impl ExtensionId<StdToolbox> for RemotingExtensionId {
  type Ext = RemotingExtensionGeneric<StdToolbox>;

  fn create_extension(&self, system: &ActorSystemGeneric<StdToolbox>) -> Self::Ext {
    RemotingExtensionGeneric::new(system, &self.config)
  }
}
