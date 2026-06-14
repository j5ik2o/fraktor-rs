use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::time::Duration;

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared, subscriber_handle},
};
use fraktor_cluster_core_kernel_rs::{
  extension::{ClusterRouterPool, ClusterRouterPoolConfig},
  membership::{CurrentClusterState, MembershipVersion, NodeRecord, NodeStatus},
  topology::{ClusterEvent, ClusterTopology, TopologyUpdate},
};
use fraktor_utils_core_rs::{
  sync::{DefaultMutex, SharedLock},
  time::TimerInstant,
};

use super::ClusterRouterPoolRouteeSubscriber;

fn node(authority: &str, status: NodeStatus) -> NodeRecord {
  node_with_roles(authority, status, Vec::new())
}

fn node_with_id(node_id: &str, authority: &str, status: NodeStatus) -> NodeRecord {
  NodeRecord::new(
    node_id.to_string(),
    authority.to_string(),
    status,
    MembershipVersion::new(1),
    String::new(),
    Vec::new(),
  )
}

fn node_with_roles(authority: &str, status: NodeStatus, roles: Vec<String>) -> NodeRecord {
  NodeRecord::new(authority.to_string(), authority.to_string(), status, MembershipVersion::new(1), String::new(), roles)
}

fn observed_at() -> TimerInstant {
  TimerInstant::zero(Duration::from_secs(1))
}

fn cluster_extension_event(event: ClusterEvent) -> EventStreamEvent {
  EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(event) }
}

fn topology_update(hash: u64, members: Vec<String>) -> TopologyUpdate {
  TopologyUpdate::new(
    ClusterTopology::new(hash, Vec::new(), Vec::new(), Vec::new()),
    members,
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(hash, Duration::from_secs(1)),
  )
}

fn routees(router: &SharedLock<ClusterRouterPool>) -> Vec<String> {
  router.with_lock(|router| router.routees().to_vec())
}

#[test]
fn subscribed_pool_updates_routees_from_cluster_events() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(1).with_allow_local_routees(false);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  let state = CurrentClusterState::new(
    vec![
      node("self:2551", NodeStatus::Up),
      node("remote-a:2552", NodeStatus::Up),
      node("remote-b:2553", NodeStatus::Joining),
    ],
    Vec::new(),
    Vec::new(),
    None,
    BTreeMap::new(),
  );
  event_stream
    .publish(&cluster_extension_event(ClusterEvent::CurrentClusterState { state, observed_at: observed_at() }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-b:2553"),
    authority:   String::from("remote-b:2553"),
    from:        NodeStatus::Joining,
    to:          NodeStatus::Up,
    observed_at: observed_at(),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552"), String::from("remote-b:2553")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-a:2552"),
    authority:   String::from("remote-a:2552"),
    from:        NodeStatus::Up,
    to:          NodeStatus::Removed,
    observed_at: observed_at(),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-b:2553")]);
}

#[test]
fn status_event_keeps_stale_snapshot_from_restoring_routee() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(1).with_allow_local_routees(false);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  let state = CurrentClusterState::new(
    vec![node("self:2551", NodeStatus::Up), node("remote-a:2552", NodeStatus::Up)],
    Vec::new(),
    Vec::new(),
    None,
    BTreeMap::new(),
  );
  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       state.clone(),
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-a:2552"),
    authority:   String::from("remote-a:2552"),
    from:        NodeStatus::Up,
    to:          NodeStatus::PreparingForShutdown,
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream
    .publish(&cluster_extension_event(ClusterEvent::CurrentClusterState { state, observed_at: observed_at() }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream.publish(&cluster_extension_event(ClusterEvent::TopologyUpdated {
    update: topology_update(2, vec![String::from("self:2551"), String::from("remote-a:2552")]),
  }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-a:2552"),
    authority:   String::from("remote-a:2552"),
    from:        NodeStatus::PreparingForShutdown,
    to:          NodeStatus::Up,
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);
}

#[test]
fn topology_updated_refreshes_routees_without_status_events() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(1).with_allow_local_routees(false);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  event_stream.publish(&cluster_extension_event(ClusterEvent::TopologyUpdated {
    update: topology_update(1, vec![String::from("self:2551"), String::from("remote-a:2552")]),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::TopologyUpdated {
    update: topology_update(2, vec![String::from("self:2551"), String::from("remote-b:2553")]),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-b:2553")]);
}

#[test]
fn topology_updated_preserves_existing_member_roles() {
  let config = ClusterRouterPoolConfig::new(10)
    .with_max_instances_per_node(1)
    .with_allow_local_routees(false)
    .with_use_roles(vec![String::from("worker")]);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![
        node("self:2551", NodeStatus::Up),
        node_with_roles("remote-a:2552", NodeStatus::Up, vec![String::from("worker")]),
      ],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::TopologyUpdated {
    update: topology_update(2, vec![String::from("self:2551"), String::from("remote-a:2552")]),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);
}

#[test]
fn status_override_does_not_apply_to_rejoined_member_with_new_node_id() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(1).with_allow_local_routees(false);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![node("self:2551", NodeStatus::Up), node_with_id("remote-a-old", "remote-a:2552", NodeStatus::Up)],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-a-old"),
    authority:   String::from("remote-a:2552"),
    from:        NodeStatus::Up,
    to:          NodeStatus::PreparingForShutdown,
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![node("self:2551", NodeStatus::Up), node_with_id("remote-a-new", "remote-a:2552", NodeStatus::Up)],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);
}

#[test]
fn snapshot_removal_clears_status_override_for_rejoined_member() {
  let config = ClusterRouterPoolConfig::new(10).with_max_instances_per_node(1).with_allow_local_routees(false);
  let router = SharedLock::new_with_driver::<DefaultMutex<_>>(ClusterRouterPool::new(config, Vec::new()));
  let event_stream = EventStreamShared::default();
  let subscriber = subscriber_handle(ClusterRouterPoolRouteeSubscriber::new(router.clone(), String::from("self:2551")));
  let _subscription = event_stream.subscribe_no_replay(&subscriber);

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![node("self:2551", NodeStatus::Up), node("remote-a:2552", NodeStatus::Up)],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);

  event_stream.publish(&cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id:     String::from("remote-a:2552"),
    authority:   String::from("remote-a:2552"),
    from:        NodeStatus::Up,
    to:          NodeStatus::PreparingForShutdown,
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![node("self:2551", NodeStatus::Up)],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));
  assert_eq!(routees(&router), Vec::<String>::new());

  event_stream.publish(&cluster_extension_event(ClusterEvent::CurrentClusterState {
    state:       CurrentClusterState::new(
      vec![node("self:2551", NodeStatus::Up), node("remote-a:2552", NodeStatus::Up)],
      Vec::new(),
      Vec::new(),
      None,
      BTreeMap::new(),
    ),
    observed_at: observed_at(),
  }));

  assert_eq!(routees(&router), vec![String::from("remote-a:2552")]);
}
