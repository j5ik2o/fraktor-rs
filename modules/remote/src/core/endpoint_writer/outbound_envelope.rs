//! Outbound envelope for remote messaging.

use fraktor_actor_rs::core::{actor_prim::actor_path::ActorPathParts, messaging::AnyMessageGeneric};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::endpoint_manager::RemoteNodeId;

/// Outbound envelope submitted to the writer.
pub struct OutboundEnvelope<TB: RuntimeToolbox + 'static> {
  /// Destination actor path.
  pub target:  ActorPathParts,
  /// Remote node metadata.
  pub remote:  RemoteNodeId,
  /// Message payload.
  pub message: AnyMessageGeneric<TB>,
}
