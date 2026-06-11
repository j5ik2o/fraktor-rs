use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use super::{DataCenter, DataCenterReachabilityTransition};
use crate::ClusterEvent;

#[test]
fn became_unreachable_converts_to_unreachable_data_center_event() {
  let dc = DataCenter::new("eu-west-1");
  let observed_at = TimerInstant::from_ticks(42, Duration::from_secs(1));
  let transition = DataCenterReachabilityTransition::BecameUnreachable { data_center: dc.clone() };

  let event = transition.to_cluster_event(observed_at);

  match event {
    | ClusterEvent::UnreachableDataCenter { data_center, observed_at: ts } => {
      assert_eq!(data_center, dc, "data_center 識別子が保持されていること");
      assert_eq!(ts.ticks(), 42, "observed_at が正しく引き継がれていること");
    },
    | other => panic!("予期するイベント: UnreachableDataCenter, 実際: {other:?}"),
  }
}

#[test]
fn became_reachable_converts_to_reachable_data_center_event() {
  let dc = DataCenter::new("ap-northeast-1");
  let observed_at = TimerInstant::from_ticks(99, Duration::from_secs(1));
  let transition = DataCenterReachabilityTransition::BecameReachable { data_center: dc.clone() };

  let event = transition.to_cluster_event(observed_at);

  match event {
    | ClusterEvent::ReachableDataCenter { data_center, observed_at: ts } => {
      assert_eq!(data_center, dc, "data_center 識別子が保持されていること");
      assert_eq!(ts.ticks(), 99, "observed_at が正しく引き継がれていること");
    },
    | other => panic!("予期するイベント: ReachableDataCenter, 実際: {other:?}"),
  }
}

#[test]
fn conversion_preserves_data_center_identifier() {
  let dc = DataCenter::new("us-east-2");
  let observed_at = TimerInstant::from_ticks(1, Duration::from_secs(1));

  let unreachable = DataCenterReachabilityTransition::BecameUnreachable { data_center: dc.clone() };
  let reachable = DataCenterReachabilityTransition::BecameReachable { data_center: dc.clone() };

  let unreachable_event = unreachable.to_cluster_event(observed_at);
  let reachable_event = reachable.to_cluster_event(observed_at);

  // data_center フィールドが正しく引き継がれることを確認する
  match unreachable_event {
    | ClusterEvent::UnreachableDataCenter { data_center, .. } => {
      assert_eq!(data_center, dc, "BecameUnreachable: data_center 識別子が保持されていること")
    },
    | other => panic!("予期しないイベント: {other:?}"),
  }
  match reachable_event {
    | ClusterEvent::ReachableDataCenter { data_center, .. } => {
      assert_eq!(data_center, dc, "BecameReachable: data_center 識別子が保持されていること")
    },
    | other => panic!("予期しないイベント: {other:?}"),
  }
}
