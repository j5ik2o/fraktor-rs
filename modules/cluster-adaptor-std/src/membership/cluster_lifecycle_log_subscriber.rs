//! Event stream subscriber that logs cluster lifecycle transitions via `tracing`.

#[cfg(test)]
#[path = "cluster_lifecycle_log_subscriber_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::event::stream::{EventStreamEvent, EventStreamSubscriber};
use fraktor_cluster_core_kernel_rs::{
  membership::NodeStatus,
  topology::{
    ClusterEvent, TRANSITION_DC_REACHABLE, TRANSITION_DC_UNREACHABLE, TRANSITION_JOIN, TRANSITION_LEAVE,
    TRANSITION_REMOVAL, TRANSITION_SHUTDOWN_PREPARING, TRANSITION_SHUTDOWN_READY, TRANSITION_UP,
  },
};
use tracing::{Level, event};

/// Extension name under which cluster events are published to the event stream.
const CLUSTER_EXTENSION_NAME: &str = "cluster";

/// Default target name used in emitted cluster lifecycle events.
const CLUSTER_LIFECYCLE_TARGET: &str = "fraktor::cluster::lifecycle";

/// Event stream subscriber that logs cluster lifecycle transitions.
///
/// Emits structured `tracing` events whose field names and transition values
/// follow the `cluster_lifecycle_trace_field` contract defined in
/// `fraktor-cluster-core-kernel-rs`. This is the fraktor counterpart of
/// Pekko's `ClusterLogMarker`-based lifecycle logging, adapted to the
/// event stream subscriber architecture.
pub struct ClusterLifecycleLogSubscriber {
  _private: (),
}

impl ClusterLifecycleLogSubscriber {
  /// Creates a new subscriber.
  #[must_use]
  pub const fn new() -> Self {
    Self { _private: () }
  }
}

impl Default for ClusterLifecycleLogSubscriber {
  fn default() -> Self {
    Self::new()
  }
}

impl EventStreamSubscriber for ClusterLifecycleLogSubscriber {
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
      | ClusterEvent::MemberStatusChanged { node_id, authority, to, .. } => {
        if let Some(kind) = status_transition_kind(to) {
          emit_member_transition(kind, node_id, authority);
        }
      },
      | ClusterEvent::MemberPreparingForShutdown { node_id, authority, .. } => {
        emit_member_transition(TRANSITION_SHUTDOWN_PREPARING, node_id, authority);
      },
      | ClusterEvent::MemberReadyForShutdown { node_id, authority, .. } => {
        emit_member_transition(TRANSITION_SHUTDOWN_READY, node_id, authority);
      },
      | ClusterEvent::UnreachableDataCenter { data_center, .. } => {
        event!(
          target: CLUSTER_LIFECYCLE_TARGET,
          Level::WARN,
          cluster.lifecycle.transition = TRANSITION_DC_UNREACHABLE,
          data_center = data_center.as_str(),
          "cluster lifecycle transition"
        );
      },
      | ClusterEvent::ReachableDataCenter { data_center, .. } => {
        event!(
          target: CLUSTER_LIFECYCLE_TARGET,
          Level::INFO,
          cluster.lifecycle.transition = TRANSITION_DC_REACHABLE,
          data_center = data_center.as_str(),
          "cluster lifecycle transition"
        );
      },
      | _ => {},
    }
  }
}

/// Maps a member status destination to a lifecycle transition kind.
///
/// Returns `None` for statuses that have no dedicated transition kind in the
/// trace field contract.
const fn status_transition_kind(to: &NodeStatus) -> Option<&'static str> {
  match to {
    | NodeStatus::Joining => Some(TRANSITION_JOIN),
    | NodeStatus::Up => Some(TRANSITION_UP),
    | NodeStatus::Leaving => Some(TRANSITION_LEAVE),
    | NodeStatus::Removed => Some(TRANSITION_REMOVAL),
    // shutdown 進行は専用イベント（MemberPreparingForShutdown / MemberReadyForShutdown）
    // 側で出力するため、status 変更経由では出力しない（二重出力の防止）。
    // WeaklyUp / Suspect / Exiting / Dead は契約上の遷移種別を持たない。
    | NodeStatus::WeaklyUp
    | NodeStatus::Suspect
    | NodeStatus::Exiting
    | NodeStatus::PreparingForShutdown
    | NodeStatus::ReadyForShutdown
    | NodeStatus::Dead => None,
  }
}

/// Emits a member-scoped lifecycle transition event.
fn emit_member_transition(kind: &'static str, node_id: &str, authority: &str) {
  event!(
    target: CLUSTER_LIFECYCLE_TARGET,
    Level::INFO,
    cluster.lifecycle.transition = kind,
    node_id = node_id,
    authority = authority,
    "cluster lifecycle transition"
  );
}
