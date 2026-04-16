//! Extension identifier for cluster runtime.

use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::{actor::extension::ExtensionId, system::ActorSystem};
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::core::{
  BlockListProvider, ClusterCore, ClusterExtension, ClusterExtensionConfig, ClusterProviderShared,
  cluster_provider::ClusterProvider,
  downing_provider::DowningProvider,
  grain::KindRegistry,
  identity::{IdentityLookup, IdentityLookupShared},
  membership::{Gossiper, GossiperShared},
  pub_sub::{ClusterPubSubShared, cluster_pub_sub::ClusterPubSub},
};

/// Registers the cluster extension into an actor system.
pub struct ClusterExtensionId {
  config:              ClusterExtensionConfig,
  provider:            ClusterProviderShared,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  downing_provider:    SharedLock<Box<dyn DowningProvider>>,
  gossiper:            GossiperShared,
  pubsub:              ClusterPubSubShared,
  identity_lookup:     IdentityLookupShared,
}

impl Clone for ClusterExtensionId {
  fn clone(&self) -> Self {
    Self {
      config:              self.config.clone(),
      provider:            self.provider.clone(),
      block_list_provider: self.block_list_provider.clone(),
      downing_provider:    self.downing_provider.clone(),
      gossiper:            self.gossiper.clone(),
      pubsub:              self.pubsub.clone(),
      identity_lookup:     self.identity_lookup.clone(),
    }
  }
}

impl ClusterExtensionId {
  /// Creates a new identifier with injected dependencies.
  ///
  /// Dependencies are wrapped in shared locks for thread-safe mutable access.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: Box<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    downing_provider: Box<dyn DowningProvider>,
    gossiper: Box<dyn Gossiper>,
    pubsub: Box<dyn ClusterPubSub>,
    identity_lookup: Box<dyn IdentityLookup>,
  ) -> Self {
    let provider = ClusterProviderShared::new(provider);
    let downing_provider = SharedLock::new_with_driver::<DefaultMutex<_>>(downing_provider);
    let gossiper = GossiperShared::new(gossiper);
    let pubsub = ClusterPubSubShared::new(pubsub);
    let identity_lookup = IdentityLookupShared::new(identity_lookup);
    Self { config, provider, block_list_provider, downing_provider, gossiper, pubsub, identity_lookup }
  }
}

impl ExtensionId for ClusterExtensionId {
  type Ext = ClusterExtension;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    let event_stream = system.event_stream();
    let kind_registry = KindRegistry::new();
    let core = ClusterCore::new(
      &self.config,
      self.provider.clone(),
      self.block_list_provider.clone(),
      event_stream,
      self.downing_provider.clone(),
      self.gossiper.clone(),
      self.pubsub.clone(),
      kind_registry,
      self.identity_lookup.clone(),
    );
    ClusterExtension::new(system, core)
  }
}
