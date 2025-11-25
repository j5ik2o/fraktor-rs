//! Installs the cluster extension into an actor system.

use fraktor_actor_rs::core::{
  extension::ExtensionInstaller,
  system::{ActorSystemBuildError, ActorSystemGeneric},
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  ClusterExtensionConfig, ClusterPubSub, Gossiper, IdentityLookup, LocalClusterProvider, NoopClusterPubSub,
  NoopGossiper, NoopIdentityLookup, cluster_extension_id::ClusterExtensionId,
};

/// Empty block list provider that never blocks any members.
#[derive(Clone, Debug, Default)]
struct EmptyBlockListProvider;

impl BlockListProvider for EmptyBlockListProvider {
  fn blocked_members(&self) -> alloc::vec::Vec<alloc::string::String> {
    alloc::vec::Vec::new()
  }
}

/// Registers the cluster extension at actor system build time.
///
/// This installer simplifies cluster setup by automatically creating default
/// implementations for `Gossiper`, `ClusterPubSub`, and `IdentityLookup` if
/// not explicitly provided.
///
/// # Example
///
/// ```text
/// use fraktor_cluster_rs::core::{ClusterExtensionConfig, ClusterExtensionInstaller, ClusterTopology};
///
/// let config = ClusterExtensionConfig::default()
///     .with_advertised_address("127.0.0.1:8080")
///     .with_metrics_enabled(true)
///     .with_static_topology(ClusterTopology::new(1, vec!["127.0.0.1:8081".into()], vec![]));
///
/// let installer = ClusterExtensionInstaller::new(config);
/// // Add to ActorSystemConfig extension_installers
/// ```
#[derive(Clone)]
pub struct ClusterExtensionInstaller {
  config:              ClusterExtensionConfig,
  block_list_provider: Option<ArcShared<dyn BlockListProvider>>,
  gossiper:            Option<ArcShared<dyn Gossiper>>,
  pubsub:              Option<ArcShared<dyn ClusterPubSub>>,
  identity_lookup:     Option<ArcShared<dyn IdentityLookup>>,
}

impl ClusterExtensionInstaller {
  /// Creates a new installer with the provided configuration.
  ///
  /// Default implementations will be used for `Gossiper`, `ClusterPubSub`,
  /// and `IdentityLookup` unless explicitly set.
  #[must_use]
  pub const fn new(config: ClusterExtensionConfig) -> Self {
    Self { config, block_list_provider: None, gossiper: None, pubsub: None, identity_lookup: None }
  }

  /// Sets a custom block list provider.
  #[must_use]
  pub fn with_block_list_provider(mut self, provider: ArcShared<dyn BlockListProvider>) -> Self {
    self.block_list_provider = Some(provider);
    self
  }

  /// Sets a custom gossiper implementation.
  #[must_use]
  pub fn with_gossiper(mut self, gossiper: ArcShared<dyn Gossiper>) -> Self {
    self.gossiper = Some(gossiper);
    self
  }

  /// Sets a custom pub/sub implementation.
  #[must_use]
  pub fn with_pubsub(mut self, pubsub: ArcShared<dyn ClusterPubSub>) -> Self {
    self.pubsub = Some(pubsub);
    self
  }

  /// Sets a custom identity lookup implementation.
  #[must_use]
  pub fn with_identity_lookup(mut self, identity_lookup: ArcShared<dyn IdentityLookup>) -> Self {
    self.identity_lookup = Some(identity_lookup);
    self
  }
}

impl ClusterExtensionInstaller {
  /// Returns the cluster extension instance, installing it if not already present.
  ///
  /// When used with `ExtensionInstallers`, the extension is installed during system build.
  /// Calling this method afterward returns the existing instance.
  ///
  /// Use this method to obtain the `ClusterExtension` for calling `start_member()`,
  /// `setup_member_kinds()`, etc.
  #[must_use]
  pub fn get<TB>(&self, system: &ActorSystemGeneric<TB>) -> ArcShared<crate::core::ClusterExtensionGeneric<TB>>
  where
    TB: RuntimeToolbox + 'static, {
    // システムの RemotingConfig から advertised address を取得（設定で未指定の場合）
    let mut config = self.config.clone();
    if config.advertised_address().is_empty() {
      if let Some(remoting_config) = system.remoting_config() {
        let addr =
          alloc::format!("{}:{}", remoting_config.canonical_host(), remoting_config.canonical_port().unwrap_or(0));
        config = config.with_advertised_address(addr);
      }
    }

    // デフォルト実装を使用（未指定の場合）
    let block_list_provider: ArcShared<dyn BlockListProvider> =
      self.block_list_provider.clone().unwrap_or_else(|| ArcShared::new(EmptyBlockListProvider));
    let gossiper: ArcShared<dyn Gossiper> = self.gossiper.clone().unwrap_or_else(|| ArcShared::new(NoopGossiper));
    let pubsub: ArcShared<dyn ClusterPubSub> = self.pubsub.clone().unwrap_or_else(|| ArcShared::new(NoopClusterPubSub));
    let identity_lookup: ArcShared<dyn IdentityLookup> =
      self.identity_lookup.clone().unwrap_or_else(|| ArcShared::new(NoopIdentityLookup));

    // LocalClusterProvider を作成
    let mut provider =
      LocalClusterProvider::new(system.event_stream(), block_list_provider.clone(), config.advertised_address());
    if let Some(topology) = config.static_topology() {
      provider = provider.with_static_topology(topology.clone());
    }
    let provider: ArcShared<dyn crate::core::ClusterProvider> = ArcShared::new(provider);

    let id = ClusterExtensionId::<TB>::new(config, provider, block_list_provider, gossiper, pubsub, identity_lookup);
    system.extended().register_extension(&id)
  }
}

impl<TB> ExtensionInstaller<TB> for ClusterExtensionInstaller
where
  TB: RuntimeToolbox + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    let _ = self.get(system);
    Ok(())
  }
}
