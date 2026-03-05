//! Command messages for the Receptionist service discovery actor.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::TypeId;

use crate::core::typed::{actor::TypedActorRef, listing::Listing};

/// Commands accepted by the Receptionist actor.
///
/// The Receptionist maintains a registry of actors indexed by a
/// `(service_id, TypeId)` pair.  Subscribers are notified whenever
/// the set of registrations for a given key changes.
pub enum ReceptionistCommand {
  /// Register an actor under a service key.
  Register {
    /// Service identifier string.
    service_id: String,
    /// Type identifier of the message type associated with the key.
    type_id:    TypeId,
    /// Erased actor reference to register.
    actor_ref:  crate::core::actor::actor_ref::ActorRef,
  },
  /// Remove a previously registered actor.
  Deregister {
    /// Service identifier string.
    service_id: String,
    /// Type identifier of the message type associated with the key.
    type_id:    TypeId,
    /// Erased actor reference to deregister.
    actor_ref:  crate::core::actor::actor_ref::ActorRef,
  },
  /// Subscribe to listing changes for a service key.
  Subscribe {
    /// Service identifier string.
    service_id: String,
    /// Type identifier of the message type associated with the key.
    type_id:    TypeId,
    /// Subscriber that receives updated listings.
    subscriber: TypedActorRef<Listing>,
  },
  /// One-shot query for current registrations under a service key.
  Find {
    /// Service identifier string.
    service_id: String,
    /// Type identifier of the message type associated with the key.
    type_id:    TypeId,
    /// Reply-to reference that receives the listing.
    reply_to:   TypedActorRef<Listing>,
  },
}
