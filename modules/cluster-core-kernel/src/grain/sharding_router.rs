//! Composition point joining extractor-based routing with the grain path.

use alloc::string::String;
use core::{any::Any, marker::PhantomData};

use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::ActorRef,
    messaging::{AnyMessage, AskResponse, AskResult},
  },
  support::futures::ActorFutureShared,
};

use super::{GrainRef, ShardingDispatchError, ShardingMessageExtractor};
use crate::{ClusterApi, activation::ClusterIdentity};

#[cfg(test)]
#[path = "sharding_router_test.rs"]
mod tests;

/// Composition point resolving grain destinations through an extractor.
///
/// Holds a grain kind and a [`ShardingMessageExtractor`], derives the
/// destination [`ClusterIdentity`] from each message, and delegates to the
/// existing [`GrainRef`] path. This router only derives and delegates — the
/// send execution (retry, codec, events, metrics) is owned by [`GrainRef`].
/// Call options and codecs are applied to the [`GrainRef`] obtained from
/// [`Self::grain_ref_for`] using its existing API.
pub struct ShardingRouter<E, M, X>
where
  X: ShardingMessageExtractor<E, M>, {
  api:       ClusterApi,
  kind:      String,
  extractor: X,
  _marker:   PhantomData<fn() -> (E, M)>,
}

impl<E, M, X> ShardingRouter<E, M, X>
where
  M: Any + Send + Sync + 'static,
  X: ShardingMessageExtractor<E, M>,
{
  /// Creates a new router for the given kind and extractor.
  #[must_use]
  pub fn new(api: ClusterApi, kind: &str, extractor: X) -> Self {
    Self { api, kind: String::from(kind), extractor, _marker: PhantomData }
  }

  /// Resolves a grain reference for the given message without sending.
  ///
  /// The resolved destination is identical to constructing
  /// [`ClusterIdentity`] with the same kind and entity id explicitly.
  ///
  /// # Errors
  ///
  /// Returns an error if the entity id cannot be derived or the derived
  /// identity is rejected by the kernel validation rules.
  pub fn grain_ref_for(&self, message: &E) -> Result<GrainRef, ShardingDispatchError> {
    let identity = self.derive_identity(message)?;
    Ok(GrainRef::new(self.api.clone(), identity))
  }

  /// Sends a message with an explicit sender through the existing grain path.
  ///
  /// # Errors
  ///
  /// Returns an error if derivation, resolution, or sending fails.
  pub fn tell_with_sender(&self, message: E, sender: &ActorRef) -> Result<(), ShardingDispatchError> {
    let (grain_ref, inner) = self.unwrap_for_dispatch(message)?;
    grain_ref.tell_with_sender(&AnyMessage::new(inner), sender).map_err(ShardingDispatchError::Call)
  }

  /// Sends a request through the existing grain path and returns the ask
  /// response.
  ///
  /// # Errors
  ///
  /// Returns an error if derivation, resolution, or sending fails.
  pub fn request(&self, message: E) -> Result<AskResponse, ShardingDispatchError> {
    let (grain_ref, inner) = self.unwrap_for_dispatch(message)?;
    grain_ref.request(&AnyMessage::new(inner)).map_err(ShardingDispatchError::Call)
  }

  /// Sends a request through the existing grain path and returns the
  /// response future.
  ///
  /// # Errors
  ///
  /// Returns an error if derivation, resolution, or sending fails.
  pub fn request_future(&self, message: E) -> Result<ActorFutureShared<AskResult>, ShardingDispatchError> {
    let (grain_ref, inner) = self.unwrap_for_dispatch(message)?;
    grain_ref.request_future(&AnyMessage::new(inner)).map_err(ShardingDispatchError::Call)
  }

  fn derive_identity(&self, message: &E) -> Result<ClusterIdentity, ShardingDispatchError> {
    let entity_id = self.extractor.entity_id(message).ok_or(ShardingDispatchError::EntityIdUnderivable)?;
    ClusterIdentity::new(self.kind.as_str(), entity_id).map_err(ShardingDispatchError::InvalidIdentity)
  }

  fn unwrap_for_dispatch(&self, message: E) -> Result<(GrainRef, M), ShardingDispatchError> {
    let identity = self.derive_identity(&message)?;
    let grain_ref = GrainRef::new(self.api.clone(), identity);
    Ok((grain_ref, self.extractor.unwrap_message(message)))
  }
}
