//! Results produced by typed cluster state subscription requests.

use fraktor_cluster_core_kernel_rs::membership::CurrentClusterState;

use crate::ClusterEventSubscription;

/// Result of applying a [`ClusterStateSubscription`](crate::ClusterStateSubscription).
pub enum ClusterStateSubscriptionResult {
  /// A cluster-event subscription was registered.
  Subscribed(ClusterEventSubscription),
  /// A cluster-event subscription was removed.
  Unsubscribed,
  /// A current cluster-state snapshot was read.
  CurrentState(CurrentClusterState),
}
