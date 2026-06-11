use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use crate::{
  membership::{DataCenter, NodeStatus},
  singleton::SingletonStuckPhase,
  topology::{ClusterEvent, ClusterEventType},
};

fn dummy_instant() -> TimerInstant {
  TimerInstant::zero(Duration::from_secs(1))
}

// --- 新 4 種別の matches テスト ---

#[test]
fn member_preparing_for_shutdown_matches_its_type() {
  let event = ClusterEvent::MemberPreparingForShutdown {
    node_id:     "node-1".into(),
    authority:   "127.0.0.1:7355".into(),
    observed_at: dummy_instant(),
  };
  assert!(ClusterEventType::MemberPreparingForShutdown.matches(&event));
  assert!(!ClusterEventType::MemberReadyForShutdown.matches(&event));
  assert!(!ClusterEventType::MemberStatusChanged.matches(&event));
}

#[test]
fn member_ready_for_shutdown_matches_its_type() {
  let event = ClusterEvent::MemberReadyForShutdown {
    node_id:     "node-1".into(),
    authority:   "127.0.0.1:7355".into(),
    observed_at: dummy_instant(),
  };
  assert!(ClusterEventType::MemberReadyForShutdown.matches(&event));
  assert!(!ClusterEventType::MemberPreparingForShutdown.matches(&event));
  assert!(!ClusterEventType::MemberStatusChanged.matches(&event));
}

#[test]
fn unreachable_data_center_matches_its_type() {
  let event =
    ClusterEvent::UnreachableDataCenter { data_center: DataCenter::new("dc-east"), observed_at: dummy_instant() };
  assert!(ClusterEventType::UnreachableDataCenter.matches(&event));
  assert!(!ClusterEventType::ReachableDataCenter.matches(&event));
  assert!(!ClusterEventType::UnreachableMember.matches(&event));
}

#[test]
fn reachable_data_center_matches_its_type() {
  let event =
    ClusterEvent::ReachableDataCenter { data_center: DataCenter::new("dc-east"), observed_at: dummy_instant() };
  assert!(ClusterEventType::ReachableDataCenter.matches(&event));
  assert!(!ClusterEventType::UnreachableDataCenter.matches(&event));
  assert!(!ClusterEventType::ReachableMember.matches(&event));
}

// --- SingletonHandOverStuck 種別照合テスト ---

#[test]
fn singleton_hand_over_stuck_matches_its_type() {
  let event = ClusterEvent::SingletonHandOverStuck {
    singleton_name: "my-singleton".into(),
    phase:          SingletonStuckPhase::HandingOver,
    observed_at:    dummy_instant(),
  };
  assert!(ClusterEventType::SingletonHandOverStuck.matches(&event));
  assert!(!ClusterEventType::UnreachableMember.matches(&event));
  assert!(!ClusterEventType::TopologyApplyFailed.matches(&event));
}

#[test]
fn singleton_hand_over_stuck_does_not_match_other_types() {
  let event = ClusterEvent::SingletonHandOverStuck {
    singleton_name: "my-singleton".into(),
    phase:          SingletonStuckPhase::BecomingOldest,
    observed_at:    dummy_instant(),
  };
  assert!(!ClusterEventType::UnreachableMember.matches(&event));
  assert!(!ClusterEventType::MemberStatusChanged.matches(&event));
  assert!(!ClusterEventType::TopologyApplyFailed.matches(&event));
}

#[test]
fn singleton_hand_over_stuck_type_does_not_match_unrelated_events() {
  let unrelated = ClusterEvent::UnreachableMember {
    node_id:     "node-1".into(),
    authority:   "127.0.0.1:7355".into(),
    observed_at: dummy_instant(),
  };
  assert!(!ClusterEventType::SingletonHandOverStuck.matches(&unrelated));
}

// --- 新旧の型が相互にマッチしないことの確認 ---

#[test]
fn shutdown_event_types_do_not_match_member_status_changed() {
  let status_event = ClusterEvent::MemberStatusChanged {
    node_id:     "node-1".into(),
    authority:   "127.0.0.1:7355".into(),
    from:        NodeStatus::Up,
    to:          NodeStatus::Leaving,
    observed_at: dummy_instant(),
  };
  assert!(!ClusterEventType::MemberPreparingForShutdown.matches(&status_event));
  assert!(!ClusterEventType::MemberReadyForShutdown.matches(&status_event));
}

#[test]
fn dc_event_types_do_not_match_member_reachability_events() {
  let unreachable_member = ClusterEvent::UnreachableMember {
    node_id:     "node-1".into(),
    authority:   "127.0.0.1:7355".into(),
    observed_at: dummy_instant(),
  };
  assert!(!ClusterEventType::UnreachableDataCenter.matches(&unreachable_member));
  assert!(!ClusterEventType::ReachableDataCenter.matches(&unreachable_member));
}
