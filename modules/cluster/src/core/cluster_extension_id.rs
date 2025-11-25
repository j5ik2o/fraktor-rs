//! Extension identifier for cluster runtime.

use alloc::boxed::Box;
use core::marker::PhantomData;

use fraktor_actor_rs::core::{extension::ExtensionId, system::ActorSystemGeneric};
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  ClusterCore, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterProvider, ClusterPubSub, Gossiper,
  IdentityLookup, KindRegistry,
};

/// Registers the cluster extension into an actor system.
pub struct ClusterExtensionId<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider:            ArcShared<ToolboxMutex<Box<dyn ClusterProvider>, TB>>,
  block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
  gossiper:            ArcShared<dyn Gossiper>,
  pubsub:              ArcShared<ToolboxMutex<Box<dyn ClusterPubSub>, TB>>,
  identity_lookup:     ArcShared<ToolboxMutex<Box<dyn IdentityLookup>, TB>>,
  _marker:             PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for ClusterExtensionId<TB> {
  fn clone(&self) -> Self {
    Self {
      config:              self.config.clone(),
      provider:            self.provider.clone(),
      block_list_provider: self.block_list_provider.clone(),
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
  /// The `identity_lookup` and `provider` are wrapped in `ToolboxMutex` for thread-safe mutable
  /// access.
  #[must_use]
  pub fn new(
    config: ClusterExtensionConfig,
    provider: Box<dyn ClusterProvider>,
    block_list_provider: ArcShared<dyn fraktor_remote_rs::core::BlockListProvider>,
    gossiper: ArcShared<dyn Gossiper>,
    pubsub: Box<dyn ClusterPubSub>,
    identity_lookup: Box<dyn IdentityLookup>,
  ) -> Self {
    let provider_mutex: ToolboxMutex<Box<dyn ClusterProvider>, TB> =
      <TB::MutexFamily as SyncMutexFamily>::create(provider);
    let pubsub_mutex: ToolboxMutex<Box<dyn ClusterPubSub>, TB> = <TB::MutexFamily as SyncMutexFamily>::create(pubsub);
    let identity_mutex: ToolboxMutex<Box<dyn IdentityLookup>, TB> =
      <TB::MutexFamily as SyncMutexFamily>::create(identity_lookup);
    Self {
      config,
      provider: ArcShared::new(provider_mutex),
      block_list_provider,
      gossiper,
      pubsub: ArcShared::new(pubsub_mutex),
      identity_lookup: ArcShared::new(identity_mutex),
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
      self.gossiper.clone(),
      self.pubsub.clone(),
      kind_registry,
      self.identity_lookup.clone(),
    );
    ClusterExtensionGeneric::new(ArcShared::new(system.clone()), core)
  }
}
