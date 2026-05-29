//! Typed cluster state subscription requests.

use alloc::vec::Vec;
use core::convert::Infallible;

use fraktor_actor_core_typed_rs::TypedActorRef;
use fraktor_cluster_core_kernel_rs::{
  extension::ClusterSubscriptionInitialStateMode,
  topology::{ClusterEvent, ClusterEventType},
};

use crate::{Cluster, ClusterStateSubscriptionResult, SelfRemoved, SelfUp};

/// Requests supported by typed cluster state subscriptions.
#[derive(Clone, Debug)]
pub enum ClusterStateSubscription {
  /// Subscribe a typed actor reference to cluster events.
  Subscribe {
    /// Typed subscriber receiving cluster events.
    subscriber:         TypedActorRef<ClusterEvent>,
    /// Initial state delivery mode.
    initial_state_mode: ClusterSubscriptionInitialStateMode,
    /// Cluster event filters.
    event_types:        Vec<ClusterEventType>,
  },
  /// Subscribe a typed actor reference to local-member up events.
  SubscribeSelfUp {
    /// Typed subscriber receiving local-member up events.
    subscriber: TypedActorRef<SelfUp>,
  },
  /// Subscribe a typed actor reference to local-member removed events.
  SubscribeSelfRemoved {
    /// Typed subscriber receiving local-member removed events.
    subscriber: TypedActorRef<SelfRemoved>,
  },
  /// Unsubscribe from cluster events by subscription identifier.
  Unsubscribe {
    /// Kernel event-stream subscription identifier.
    subscription_id: u64,
  },
  /// Read the current cluster state snapshot.
  GetCurrentState,
}

impl ClusterStateSubscription {
  /// Applies this request to the supplied typed cluster facade.
  ///
  /// # Errors
  ///
  /// This request currently cannot fail because the facade already owns a
  /// resolved kernel cluster API.
  pub fn apply_to(self, cluster: &Cluster) -> Result<ClusterStateSubscriptionResult, Infallible> {
    Ok(match self {
      | Self::Subscribe { subscriber, initial_state_mode, event_types } => {
        ClusterStateSubscriptionResult::Subscribed(cluster.subscribe(&subscriber, initial_state_mode, &event_types))
      },
      | Self::SubscribeSelfUp { subscriber } => {
        ClusterStateSubscriptionResult::Subscribed(cluster.subscribe_self_up(&subscriber))
      },
      | Self::SubscribeSelfRemoved { subscriber } => {
        ClusterStateSubscriptionResult::Subscribed(cluster.subscribe_self_removed(&subscriber))
      },
      | Self::Unsubscribe { subscription_id } => {
        cluster.unsubscribe(subscription_id);
        ClusterStateSubscriptionResult::Unsubscribed
      },
      | Self::GetCurrentState => ClusterStateSubscriptionResult::CurrentState(cluster.current_state()),
    })
  }
}
