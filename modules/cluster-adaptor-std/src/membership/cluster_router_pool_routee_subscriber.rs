//! Cluster router pool routee updates driven by cluster events.

#[cfg(test)]
#[path = "cluster_router_pool_routee_subscriber_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::event::stream::{
  EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, subscriber_handle,
};
use fraktor_cluster_core_kernel_rs::{
  extension::{ClusterApi, ClusterRouterPool, ClusterSubscriptionInitialStateMode},
  membership::{MembershipVersion, NodeRecord, NodeStatus},
  topology::{ClusterEvent, ClusterEventType},
};
use fraktor_utils_core_rs::sync::SharedLock;

const CLUSTER_EXTENSION_NAME: &str = "cluster";

/// Event stream subscriber that keeps a cluster router pool in sync with membership.
pub struct ClusterRouterPoolRouteeSubscriber {
  router:           SharedLock<ClusterRouterPool>,
  self_authority:   String,
  members:          Vec<NodeRecord>,
  status_overrides: Vec<(String, NodeStatus)>,
}

impl ClusterRouterPoolRouteeSubscriber {
  /// Creates a subscriber for the provided router and local authority.
  #[must_use]
  pub const fn new(router: SharedLock<ClusterRouterPool>, self_authority: String) -> Self {
    Self { router, self_authority, members: Vec::new(), status_overrides: Vec::new() }
  }

  /// Subscribes the router pool to cluster state and member-status events.
  #[must_use]
  pub fn subscribe(cluster: &ClusterApi, router: SharedLock<ClusterRouterPool>) -> EventStreamSubscription {
    let subscriber = subscriber_handle(Self::new(router, cluster.self_authority()));
    cluster.subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsSnapshot, &[
      ClusterEventType::CurrentClusterState,
      ClusterEventType::TopologyUpdated,
      ClusterEventType::MemberStatusChanged,
    ])
  }

  fn replace_members(&mut self, members: &[NodeRecord]) {
    self.members = members.to_vec();
    self.retain_status_overrides_for_current_members();
    self.apply_status_overrides();
    self.update_router();
  }

  fn replace_member_authorities(&mut self, authorities: &[String], version: MembershipVersion) {
    self.members = authorities
      .iter()
      .cloned()
      .map(|authority| {
        let node_id = authority.clone();
        NodeRecord::new(node_id, authority, NodeStatus::Up, version, String::new(), Vec::new())
      })
      .collect();
    self.retain_status_overrides_for_current_members();
    self.apply_status_overrides();
    self.update_router();
  }

  fn apply_member_status(&mut self, node_id: &str, authority: &str, to: NodeStatus) {
    self.record_status_override(authority, to);
    if let Some(member) = self.members.iter_mut().find(|member| member.authority == authority) {
      member.status = to;
    } else {
      self.members.push(NodeRecord::new(
        String::from(node_id),
        String::from(authority),
        to,
        MembershipVersion::new(0),
        String::new(),
        Vec::new(),
      ));
    }
    self.update_router();
  }

  fn apply_status_overrides(&mut self) {
    for member in &mut self.members {
      if let Some((_, status)) = self.status_overrides.iter().find(|(authority, _)| authority == &member.authority) {
        member.status = *status;
      }
    }
  }

  fn retain_status_overrides_for_current_members(&mut self) {
    let members = &self.members;
    self.status_overrides.retain(|(authority, _)| members.iter().any(|member| &member.authority == authority));
  }

  fn record_status_override(&mut self, authority: &str, status: NodeStatus) {
    if status == NodeStatus::Up {
      self.status_overrides.retain(|(recorded_authority, _)| recorded_authority != authority);
      return;
    }
    if let Some((_, recorded_status)) =
      self.status_overrides.iter_mut().find(|(recorded_authority, _)| recorded_authority == authority)
    {
      *recorded_status = status;
      return;
    }
    self.status_overrides.push((String::from(authority), status));
  }

  fn update_router(&self) {
    self.router.with_lock(|router| router.update_from_members(&self.members, &self.self_authority));
  }
}

impl EventStreamSubscriber for ClusterRouterPoolRouteeSubscriber {
  fn on_event(&mut self, stream_event: &EventStreamEvent) {
    let EventStreamEvent::Extension { name, payload } = stream_event else {
      return;
    };
    if name != CLUSTER_EXTENSION_NAME {
      return;
    }
    let Some(cluster_event) = payload.downcast_ref::<ClusterEvent>() else {
      return;
    };
    match cluster_event {
      | ClusterEvent::CurrentClusterState { state, .. } => self.replace_members(&state.members),
      | ClusterEvent::TopologyUpdated { update } => {
        self.replace_member_authorities(&update.members, MembershipVersion::new(update.topology.hash()));
      },
      | ClusterEvent::MemberStatusChanged { node_id, authority, to, .. } => {
        self.apply_member_status(node_id, authority, *to);
      },
      | _ => {},
    }
  }
}
