//! Extension identifier for cluster runtime.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  ClusterCore, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterProviderShared,
  cluster_provider::ClusterProvider,
  downing_provider::DowningProvider,
  grain::KindRegistry,
  identity::{IdentityLookup, IdentityLookupShared},
  membership::{Gossiper, GossiperShared},
  pub_sub::{ClusterPubSubShared, cluster_pub_sub::ClusterPubSub},
};

/// Registers the cluster extension into an actor system.
pub struct ClusterExtensionId<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider:            ClusterProviderShared<TB>,
  block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
  downing_provider:    ArcShared<ToolboxMutex<Box<dyn DowningProvider>, TB>>,
  gossiper:            GossiperShared<TB>,
  pubsub:              ClusterPubSubShared<TB>,
  identity_lookup:     IdentityLookupShared<TB>,
  _marker:             PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for ClusterExtensionId<TB> {
  fn clone(&self) -> Self {
    Self {
      config:              self.config.clone(),
      provider:            self.provider.clone(),
      block_list_provider: self.block_list_provider.clone(),
      downing_provider:    self.downing_provider.clone(),
      gossiper:            self.gossiper.clone(),
      pubsub:              self.pubsub.clone(),
      identity_lookup:     self.identity_lookup.clone(),
      _marker:             PhantomData,
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionId<TB> {
  /// Creates a new identifier with injected dependencies.
  ///
  /// The `identity_lookup`, `provider`, `gossiper`, and `pubsub` are wrapped in `ToolboxMutex`
  /// for thread-safe mutable access.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: Box<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
    downing_provider: Box<dyn DowningProvider>,
    gossiper: Box<dyn Gossiper>,
    pubsub: Box<dyn ClusterPubSub<TB>>,
    identity_lookup: Box<dyn IdentityLookup>,
  ) -> Self {
    let provider = ClusterProviderShared::new(provider);
    let downing_provider = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(downing_provider));
    let gossiper = GossiperShared::new(gossiper);
    let pubsub = ClusterPubSubShared::new(pubsub);
    let identity_lookup = IdentityLookupShared::new(identity_lookup);
    Self {
      config,
      provider,
      block_list_provider,
      downing_provider,
      gossiper,
      pubsub,
      identity_lookup,
      _marker: PhantomData,
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for ClusterExtensionId<TB> {
  type Ext = ClusterExtensionGeneric<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
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
    ClusterExtensionGeneric::new(system, core)
  }
}
