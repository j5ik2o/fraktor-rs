//! Extension identifier bridging actor-system registry and remoting implementation.

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystem};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::remoting_extension::{RemotingExtensionConfig, RemotingExtensionGeneric};

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
  type Ext = RemotingExtensionGeneric<StdToolbox>;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    RemotingExtensionGeneric::new(system, &self.config)
  }
}
