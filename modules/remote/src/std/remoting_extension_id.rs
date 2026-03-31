//! Extension identifier bridging actor-system registry and remoting implementation.

use fraktor_actor_rs::core::kernel::{actor::extension::ExtensionId, system::ActorSystem};

use crate::core::remoting_extension::{RemotingExtension, RemotingExtensionConfig};

/// Registers and instantiates [`crate::core::RemotingExtension`] instances.
///
/// This type is only available with the `std` feature because extension
/// initialization requires transport implementations backed by standard facilities.
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

impl ExtensionId for RemotingExtensionId {
  type Ext = RemotingExtension;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    RemotingExtension::new(system, &self.config)
  }
}
