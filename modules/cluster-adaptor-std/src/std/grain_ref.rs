//! Std wrapper for grain references.

use fraktor_actor_core_rs::actor::{
  actor_ref::ActorRef,
  messaging::{AnyMessage, AskResponse},
};
use fraktor_cluster_core_rs::core::{
  grain::{GrainCallError, GrainCallOptions, GrainCodec, GrainRef as CoreGrainRef},
  identity::ClusterIdentity,
};
use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::std::ClusterApi;

/// Grain reference bound to the standard toolbox.
pub struct GrainRef {
  inner: CoreGrainRef,
}

impl GrainRef {
  /// Creates a new grain reference.
  pub fn new(api: ClusterApi, identity: ClusterIdentity) -> Self {
    Self { inner: CoreGrainRef::new(api.into_core(), identity) }
  }

  /// Applies call options to the grain reference.
  #[must_use]
  pub fn with_options(mut self, options: GrainCallOptions) -> Self {
    self.inner = self.inner.with_options(options);
    self
  }

  /// Attaches a codec to validate serialization.
  #[must_use]
  pub fn with_codec(mut self, codec: ArcShared<dyn GrainCodec>) -> Self {
    self.inner = self.inner.with_codec(codec);
    self
  }

  /// Sends a request with an explicit sender and returns the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution or sending fails.
  pub fn request_with_sender(&self, message: &AnyMessage, sender: &ActorRef) -> Result<AskResponse, GrainCallError> {
    self.inner.request_with_sender(message, sender)
  }
}
