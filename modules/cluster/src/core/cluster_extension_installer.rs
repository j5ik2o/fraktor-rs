//! Installs the cluster extension into an actor system.

use alloc::boxed::Box;

use fraktor_actor_rs::core::{
  event_stream::EventStreamGeneric,
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  ClusterExtensionConfig, ClusterProvider, ClusterPubSub, Gossiper, IdentityLookup, LocalClusterProvider,
  NoopClusterPubSub, NoopGossiper, NoopIdentityLookup, cluster_extension_id::ClusterExtensionId,
};

/// Empty block list provider that never blocks any members.
#[derive(Clone, Debug, Default)]
struct EmptyBlockListProvider;

impl BlockListProvider for EmptyBlockListProvider {
  fn blocked_members(&self) -> alloc::vec::Vec<alloc::string::String> {
    alloc::vec::Vec::new()
  }
}

/// Factory function type for creating a `ClusterProvider`.
///
/// This function receives the necessary dependencies and returns a `ClusterProvider` instance.
///
/// # Arguments
/// - `event_stream` - The actor system's event stream for publishing cluster events
/// - `block_list_provider` - Provider for blocked member information
/// - `advertised_address` - The address this node advertises to the cluster
pub type ClusterProviderFactory<TB> = ArcShared<
  dyn Fn(ArcShared<EventStreamGeneric<TB>>, ArcShared<dyn BlockListProvider>, &str) -> Box<dyn ClusterProvider>
    + Send
    + Sync,
>;

/// Factory function type for creating a `Gossiper`.
type GossiperFactory = ArcShared<dyn Fn() -> Box<dyn Gossiper> + Send + Sync>;

/// Factory function type for creating a `ClusterPubSub`.
type PubSubFactory = ArcShared<dyn Fn() -> Box<dyn ClusterPubSub> + Send + Sync>;

/// Factory function type for creating an `IdentityLookup`.
type IdentityLookupFactory = ArcShared<dyn Fn() -> Box<dyn IdentityLookup> + Send + Sync>;

/// Registers the cluster extension at actor system build time.
///
/// This installer simplifies cluster setup by automatically creating default
/// implementations for `Gossiper`, `ClusterPubSub`, and `IdentityLookup` if
/// not explicitly provided.
///
/// # Example
///
/// ```text
/// use fraktor_cluster_rs::core::{ClusterExtensionConfig, ClusterExtensionInstaller};
///
/// let config = ClusterExtensionConfig::default()
///     .with_advertised_address("127.0.0.1:8080")
///     .with_metrics_enabled(true);
///
/// // Use new_with_local for LocalClusterProvider (convenience)
/// let installer = ClusterExtensionInstaller::new_with_local(config);
///
/// // Or use new with a custom ClusterProvider factory
/// // let installer = ClusterExtensionInstaller::new(config, |event_stream, block_list, addr| {
/// //     ArcShared::new(MyCustomProvider::new(event_stream, block_list, addr))
/// // });
///
/// // Add to ActorSystemConfig extension_installers
/// ```
pub struct ClusterExtensionInstaller<TB: RuntimeToolbox + 'static> {
  config:              ClusterExtensionConfig,
  provider_f:          ClusterProviderFactory<TB>,
  block_list_provider: Option<ArcShared<dyn BlockListProvider>>,
  gossiper_f:          Option<GossiperFactory>,
  pubsub_f:            Option<PubSubFactory>,
  identity_lookup_f:   Option<IdentityLookupFactory>,
}

impl<TB: RuntimeToolbox + 'static> Clone for ClusterExtensionInstaller<TB> {
  fn clone(&self) -> Self {
    Self {
      config:              self.config.clone(),
      provider_f:          ArcShared::clone(&self.provider_f),
      block_list_provider: self.block_list_provider.clone(),
      gossiper_f:          self.gossiper_f.clone(),
      pubsub_f:            self.pubsub_f.clone(),
      identity_lookup_f:   self.identity_lookup_f.clone(),
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionInstaller<TB> {
  /// Creates a new installer with the provided configuration and cluster provider factory.
  ///
  /// Use this constructor when you need a custom `ClusterProvider` implementation
  /// (e.g., etcd, zookeeper, or other service discovery providers).
  ///
  /// # Arguments
  /// - `config` - Cluster extension configuration
  /// - `provider_f` - Factory function that creates the `ClusterProvider`
  ///
  /// # Example
  ///
  /// ```text
  /// let installer = ClusterExtensionInstaller::new(config, |event_stream, block_list, addr| {
  ///     Box::new(MyEtcdProvider::new(event_stream, block_list, addr))
  /// });
  /// ```
  ///
  /// Default implementations will be used for `Gossiper`, `ClusterPubSub`,
  /// and `IdentityLookup` unless explicitly set.
  #[must_use]
  pub fn new<F>(config: ClusterExtensionConfig, provider_f: F) -> Self
  where
    F: Fn(ArcShared<EventStreamGeneric<TB>>, ArcShared<dyn BlockListProvider>, &str) -> Box<dyn ClusterProvider>
      + Send
      + Sync
      + 'static, {
    Self {
      config,
      provider_f: ArcShared::new(provider_f),
      block_list_provider: None,
      gossiper_f: None,
      pubsub_f: None,
      identity_lookup_f: None,
    }
  }

  /// Creates a new installer with `LocalClusterProvider`.
  ///
  /// This is a convenience constructor for the common case where you want to use
  /// the built-in `LocalClusterProvider` with EventStream-based topology management.
  ///
  /// Default implementations will be used for `Gossiper`, `ClusterPubSub`,
  /// and `IdentityLookup` unless explicitly set.
  #[must_use]
  pub fn new_with_local(config: ClusterExtensionConfig) -> Self {
    let static_topology = config.static_topology().cloned();
    Self::new(config, move |event_stream, block_list_provider, advertised_address| {
      let mut provider = LocalClusterProvider::new(event_stream, block_list_provider, advertised_address);
      if let Some(ref topology) = static_topology {
        provider = provider.with_static_topology(topology.clone());
      }
      Box::new(provider)
    })
  }

  /// Creates a new installer with `AwsEcsClusterProvider`.
  ///
  /// This is a convenience constructor for AWS ECS environments where task discovery
  /// is performed via the ECS API (ListTasks + DescribeTasks).
  ///
  /// Requires the `aws-ecs` feature to be enabled.
  ///
  /// # Example
  ///
  /// ```text
  /// use fraktor_cluster_rs::core::{ClusterExtensionConfig, ClusterExtensionInstaller};
  /// use fraktor_cluster_rs::std::EcsClusterConfig;
  /// use std::time::Duration;
  ///
  /// let ecs_config = EcsClusterConfig::new()
  ///     .with_cluster_name("my-cluster")
  ///     .with_service_name("my-service")
  ///     .with_poll_interval(Duration::from_secs(10));
  ///
  /// let installer = ClusterExtensionInstaller::new_with_ecs(
  ///     ClusterExtensionConfig::default().with_advertised_address("10.0.0.1:8080"),
  ///     ecs_config,
  /// );
  /// ```
  #[cfg(feature = "aws-ecs")]
  #[must_use]
  pub fn new_with_ecs(
    config: ClusterExtensionConfig,
    ecs_config: crate::std::EcsClusterConfig,
  ) -> ClusterExtensionInstaller<fraktor_utils_rs::std::runtime_toolbox::StdToolbox> {
    ClusterExtensionInstaller::new(config, move |event_stream, block_list_provider, advertised_address| {
      Box::new(
        crate::std::AwsEcsClusterProvider::new(event_stream, block_list_provider, advertised_address)
          .with_ecs_config(ecs_config.clone()),
      )
    })
  }

  /// Sets a custom block list provider.
  #[must_use]
  pub fn with_block_list_provider(mut self, provider: ArcShared<dyn BlockListProvider>) -> Self {
    self.block_list_provider = Some(provider);
    self
  }

  /// Sets a custom gossiper factory.
  ///
  /// The factory is called during installation to create a fresh `Gossiper` instance.
  #[must_use]
  pub fn with_gossiper_factory<F>(mut self, factory: F) -> Self
  where
    F: Fn() -> Box<dyn Gossiper> + Send + Sync + 'static, {
    self.gossiper_f = Some(ArcShared::new(factory));
    self
  }

  /// Sets a custom pub/sub factory.
  ///
  /// The factory is called during installation to create a fresh `ClusterPubSub` instance.
  #[must_use]
  pub fn with_pubsub_factory<F>(mut self, factory: F) -> Self
  where
    F: Fn() -> Box<dyn ClusterPubSub> + Send + Sync + 'static, {
    self.pubsub_f = Some(ArcShared::new(factory));
    self
  }

  /// Sets a custom identity lookup factory.
  ///
  /// The factory is called during installation to create a fresh `IdentityLookup` instance.
  #[must_use]
  pub fn with_identity_lookup_factory<F>(mut self, factory: F) -> Self
  where
    F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
    self.identity_lookup_f = Some(ArcShared::new(factory));
    self
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterExtensionInstaller<TB> {
  /// Installs the cluster extension into the actor system.
  ///
  /// This is a **command** that creates and registers the `ClusterExtension`.
  /// Call this method once after system creation to install the extension.
  ///
  /// Returns the installed `ClusterExtension` instance for immediate use.
  ///
  /// # Panics
  ///
  /// Panics if the extension is already installed with different configuration.
  #[must_use]
  pub fn install(&self, system: &ActorSystemGeneric<TB>) -> ArcShared<crate::core::ClusterExtensionGeneric<TB>> {
    // システムの RemotingConfig から advertised address を取得（設定で未指定の場合）
    let mut config = self.config.clone();
    if config.advertised_address().is_empty()
      && let Some(remoting_config) = system.remoting_config()
    {
      let addr =
        alloc::format!("{}:{}", remoting_config.canonical_host(), remoting_config.canonical_port().unwrap_or(0));
      config = config.with_advertised_address(addr);
    }

    // デフォルト実装を使用（未指定の場合）
    let block_list_provider: ArcShared<dyn BlockListProvider> =
      self.block_list_provider.clone().unwrap_or_else(|| ArcShared::new(EmptyBlockListProvider));
    // Gossiper はファクトリ経由で作成（Clone できないため）
    let gossiper: Box<dyn Gossiper> = self.gossiper_f.as_ref().map(|f| f()).unwrap_or_else(|| Box::new(NoopGossiper));
    // ClusterPubSub はファクトリ経由で作成（Clone できないため）
    let pubsub: Box<dyn ClusterPubSub> =
      self.pubsub_f.as_ref().map(|f| f()).unwrap_or_else(|| Box::new(NoopClusterPubSub));
    // IdentityLookup はファクトリ経由で作成（Clone できないため）
    let identity_lookup: Box<dyn IdentityLookup> =
      self.identity_lookup_f.as_ref().map(|f| f()).unwrap_or_else(|| Box::new(NoopIdentityLookup));

    // ファクトリー関数を呼び出して ClusterProvider を作成
    let provider = (self.provider_f)(system.event_stream(), block_list_provider.clone(), config.advertised_address());

    let id = ClusterExtensionId::<TB>::new(config, provider, block_list_provider, gossiper, pubsub, identity_lookup);
    system.extended().register_extension(&id)
  }
}

impl<TB> ExtensionInstaller<TB> for ClusterExtensionInstaller<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let _ = ClusterExtensionInstaller::install(self, system);
    Ok(())
  }
}
