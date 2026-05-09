//! Pub/Sub subscriber identifier.

use alloc::{format, string::String};
use core::{
  cmp::Ordering,
  hash::{Hash, Hasher},
};

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;

use crate::core::identity::ClusterIdentity;

/// Subscriber target for pub/sub delivery.
#[derive(Debug)]
pub enum PubSubSubscriber {
  /// Local actor reference.
  ActorRef(ActorRef),
  /// Cluster identity (resolved via cluster routing).
  ClusterIdentity(ClusterIdentity),
}

impl Clone for PubSubSubscriber {
  fn clone(&self) -> Self {
    match self {
      | Self::ActorRef(actor_ref) => Self::ActorRef(actor_ref.clone()),
      | Self::ClusterIdentity(identity) => Self::ClusterIdentity(identity.clone()),
    }
  }
}

impl PartialEq for PubSubSubscriber {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      | (Self::ActorRef(left), Self::ActorRef(right)) => left.pid() == right.pid(),
      | (Self::ClusterIdentity(left), Self::ClusterIdentity(right)) => {
        left.kind() == right.kind() && left.identity() == right.identity()
      },
      | _ => false,
    }
  }
}

impl Eq for PubSubSubscriber {}

impl Hash for PubSubSubscriber {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      | Self::ActorRef(actor_ref) => {
        0u8.hash(state);
        let pid = actor_ref.pid();
        pid.value().hash(state);
        pid.generation().hash(state);
      },
      | Self::ClusterIdentity(identity) => {
        1u8.hash(state);
        identity.kind().hash(state);
        identity.identity().hash(state);
      },
    }
  }
}

impl PubSubSubscriber {
  /// Returns a display label for observability.
  #[must_use]
  pub fn label(&self) -> String {
    match self {
      | Self::ActorRef(actor_ref) => format!("{}", actor_ref.pid()),
      | Self::ClusterIdentity(identity) => format!("{}/{}", identity.kind(), identity.identity()),
    }
  }
}

impl PartialOrd for PubSubSubscriber {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PubSubSubscriber {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      | (Self::ActorRef(left), Self::ActorRef(right)) => {
        let left_pid = left.pid();
        let right_pid = right.pid();
        left_pid.value().cmp(&right_pid.value()).then(left_pid.generation().cmp(&right_pid.generation()))
      },
      | (Self::ClusterIdentity(left), Self::ClusterIdentity(right)) => {
        left.kind().cmp(right.kind()).then(left.identity().cmp(right.identity()))
      },
      | (Self::ActorRef(_), Self::ClusterIdentity(_)) => Ordering::Less,
      | (Self::ClusterIdentity(_), Self::ActorRef(_)) => Ordering::Greater,
    }
  }
}
