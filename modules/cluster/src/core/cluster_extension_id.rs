//! Extension identifier for cluster runtime.

use core::marker::PhantomData;

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  ClusterCore, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterProvider, ClusterPubSub, Gossiper,
  IdentityLookup, KindRegistry,
};

/// Registers the cluster extension into an actor system.
#[derive(Clone)]
pub struct ClusterExtensionId<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider:            ArcShared<dyn ClusterProvider>,
  block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
  gossiper:            ArcShared<dyn Gossiper>,
  pubsub:              ArcShared<dyn ClusterPubSub>,
  identity_lookup:     ArcShared<dyn IdentityLookup>,
  _marker:             PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionId<TB> {
  /// Creates a new identifier with injected dependencies.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: ArcShared<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
    gossiper: ArcShared<dyn Gossiper>,
    pubsub: ArcShared<dyn ClusterPubSub>,
    identity_lookup: ArcShared<dyn IdentityLookup>,
  ) -> Self {
    Self { config, provider, block_list_provider, gossiper, pubsub, identity_lookup, _marker: PhantomData }
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
      self.gossiper.clone(),
      self.pubsub.clone(),
      kind_registry,
      self.identity_lookup.clone(),
    );
    ClusterExtensionGeneric::new(ArcShared::new(system.clone()), core)
  }
}
