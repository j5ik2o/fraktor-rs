//! Typed distributed-data extension facade.

#[cfg(test)]
#[path = "distributed_data_test.rs"]
mod tests;

use core::time::Duration;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{Extension, ExtensionId},
  system::ActorSystem,
};
use fraktor_actor_core_typed_rs::TypedActorSystem;
use fraktor_cluster_core_kernel_rs::{
  ddata::SelfUniqueAddress,
  extension::{ClusterApi, ClusterApiError, ClusterExtension},
};
use fraktor_utils_core_rs::sync::ArcShared;

/// Default timeout used by [`ReplicatorMessageAdapter`](crate::ReplicatorMessageAdapter) ask
/// operations.
pub const DEFAULT_UNEXPECTED_ASK_TIMEOUT: Duration = Duration::from_secs(60);

/// Identifier for the typed [`DistributedData`] extension.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct DistributedDataId;

/// Typed facade for distributed-data configuration and kernel vocabulary access.
///
/// This is the fraktor equivalent of Pekko's typed `DistributedData` extension.
/// It delegates self-node identity to kernel [`SelfUniqueAddress`] and exposes
/// kernel distributed-data protocol types without duplicating CRDT logic.
pub struct DistributedData {
  self_unique_address:    SelfUniqueAddress,
  unexpected_ask_timeout: Duration,
  extension:              ArcShared<ClusterExtension>,
}

impl DistributedDataId {
  /// Creates a new distributed-data extension identifier.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Extension for DistributedData {}

impl ExtensionId for DistributedDataId {
  type Ext = DistributedData;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    Self::Ext::try_from_system(system).unwrap_or_else(|error| {
      panic!("cluster extension must be installed before distributed data: {error:?}");
    })
  }
}

impl DistributedData {
  /// Retrieves the typed distributed-data facade from a typed actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn get<M>(system: &TypedActorSystem<M>) -> Result<Self, ClusterApiError>
  where
    M: Send + Sync + 'static, {
    Self::try_from_system(system.as_untyped())
  }

  /// Retrieves the typed distributed-data facade from an actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn try_from_system(system: &ActorSystem) -> Result<Self, ClusterApiError> {
    let api = ClusterApi::try_from_system(system)?;
    let extension =
      system.extended().extension_by_type::<ClusterExtension>().ok_or(ClusterApiError::ExtensionNotInstalled)?;
    Ok(Self {
      self_unique_address: SelfUniqueAddress::from_authority(&api.self_authority()),
      unexpected_ask_timeout: DEFAULT_UNEXPECTED_ASK_TIMEOUT,
      extension,
    })
  }

  /// Returns the local node identity used by CRDT updates.
  #[must_use]
  pub fn self_unique_address(&self) -> &SelfUniqueAddress {
    &self.self_unique_address
  }

  /// Returns the timeout used by [`ReplicatorMessageAdapter`](crate::ReplicatorMessageAdapter) ask
  /// operations.
  #[must_use]
  pub const fn unexpected_ask_timeout(&self) -> Duration {
    self.unexpected_ask_timeout
  }

  /// Returns the installed cluster extension backing this facade.
  #[must_use]
  pub fn cluster_extension(&self) -> &ArcShared<ClusterExtension> {
    &self.extension
  }
}
