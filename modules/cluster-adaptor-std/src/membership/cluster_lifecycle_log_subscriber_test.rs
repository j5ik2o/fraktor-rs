use core::{fmt::Debug, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamSubscriber},
};
use fraktor_cluster_core_kernel_rs::{
  membership::{DataCenter, NodeStatus},
  topology::{
    ClusterEvent, FIELD_AUTHORITY, FIELD_DATA_CENTER, FIELD_NODE_ID, FIELD_TRANSITION, TRANSITION_DC_REACHABLE,
    TRANSITION_DC_UNREACHABLE, TRANSITION_JOIN, TRANSITION_LEAVE, TRANSITION_REMOVAL, TRANSITION_SHUTDOWN_PREPARING,
    TRANSITION_SHUTDOWN_READY, TRANSITION_UP,
  },
};
use fraktor_utils_core_rs::{
  sync::{DefaultMutex, SharedAccess, SharedLock},
  time::TimerInstant,
};
use tracing::{
  Event, Metadata, Subscriber,
  field::{Field, Visit},
  span::{Attributes, Id, Record},
  subscriber::with_default,
};

use super::ClusterLifecycleLogSubscriber;

// --- tracing イベントのキャプチャ基盤（テスト専用） ---

struct RecordedEvent {
  fields: Vec<(String, String)>,
}

struct FieldCollector {
  fields: Vec<(String, String)>,
}

impl Visit for FieldCollector {
  fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
    self.fields.push((field.name().to_string(), format!("{value:?}")));
  }

  fn record_str(&mut self, field: &Field, value: &str) {
    self.fields.push((field.name().to_string(), value.to_string()));
  }
}

struct CapturingSubscriber {
  events: SharedLock<Vec<RecordedEvent>>,
}

impl Subscriber for CapturingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
    Id::from_u64(1)
  }

  fn record(&self, _id: &Id, _record: &Record<'_>) {}

  fn record_follows_from(&self, _id: &Id, _follows: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let mut collector = FieldCollector { fields: Vec::new() };
    event.record(&mut collector);
    let recorded = RecordedEvent { fields: collector.fields };
    self.events.with_write(|events| events.push(recorded));
  }

  fn enter(&self, _id: &Id) {}

  fn exit(&self, _id: &Id) {}
}

fn capture_events(stream_event: &EventStreamEvent) -> Vec<Vec<(String, String)>> {
  let events = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new());
  let capturing = CapturingSubscriber { events: events.clone() };
  with_default(capturing, || {
    let mut subscriber = ClusterLifecycleLogSubscriber::new();
    subscriber.on_event(stream_event);
  });
  events.with_read(|recorded| recorded.iter().map(|event| event.fields.clone()).collect())
}

fn cluster_extension_event(event: ClusterEvent) -> EventStreamEvent {
  EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(event) }
}

fn member_status_changed(to: NodeStatus) -> EventStreamEvent {
  cluster_extension_event(ClusterEvent::MemberStatusChanged {
    node_id: String::from("node-1"),
    authority: String::from("127.0.0.1:7000"),
    from: NodeStatus::Joining,
    to,
    observed_at: TimerInstant::from_ticks(1, Duration::from_secs(1)),
  })
}

fn contains(fields: &[(String, String)], key: &str, value: &str) -> bool {
  fields.iter().any(|(name, recorded)| name == key && recorded == value)
}

// --- テスト本体 ---

#[test]
fn member_up_transition_emits_contract_fields() {
  let captured = capture_events(&member_status_changed(NodeStatus::Up));

  assert_eq!(captured.len(), 1);
  let fields = &captured[0];
  assert!(contains(fields, FIELD_TRANSITION, TRANSITION_UP));
  assert!(contains(fields, FIELD_NODE_ID, "node-1"));
  assert!(contains(fields, FIELD_AUTHORITY, "127.0.0.1:7000"));
}

#[test]
fn join_leave_removal_transitions_map_to_contract_values() {
  let cases = [
    (NodeStatus::Joining, TRANSITION_JOIN),
    (NodeStatus::Leaving, TRANSITION_LEAVE),
    (NodeStatus::Removed, TRANSITION_REMOVAL),
  ];
  for (status, expected) in cases {
    let captured = capture_events(&member_status_changed(status));
    assert_eq!(captured.len(), 1);
    assert!(contains(&captured[0], FIELD_TRANSITION, expected));
  }
}

#[test]
fn shutdown_progress_events_emit_dedicated_transitions() {
  let observed_at = TimerInstant::from_ticks(2, Duration::from_secs(1));
  let preparing = cluster_extension_event(ClusterEvent::MemberPreparingForShutdown {
    node_id: String::from("node-2"),
    authority: String::from("127.0.0.1:7001"),
    observed_at,
  });
  let ready = cluster_extension_event(ClusterEvent::MemberReadyForShutdown {
    node_id: String::from("node-2"),
    authority: String::from("127.0.0.1:7001"),
    observed_at,
  });

  let captured_preparing = capture_events(&preparing);
  assert_eq!(captured_preparing.len(), 1);
  assert!(contains(&captured_preparing[0], FIELD_TRANSITION, TRANSITION_SHUTDOWN_PREPARING));
  assert!(contains(&captured_preparing[0], FIELD_NODE_ID, "node-2"));

  let captured_ready = capture_events(&ready);
  assert_eq!(captured_ready.len(), 1);
  assert!(contains(&captured_ready[0], FIELD_TRANSITION, TRANSITION_SHUTDOWN_READY));
}

#[test]
fn data_center_reachability_events_emit_data_center_field() {
  let observed_at = TimerInstant::from_ticks(3, Duration::from_secs(1));
  let unreachable = cluster_extension_event(ClusterEvent::UnreachableDataCenter {
    data_center: DataCenter::new("dc-east"),
    observed_at,
  });
  let reachable =
    cluster_extension_event(ClusterEvent::ReachableDataCenter { data_center: DataCenter::new("dc-east"), observed_at });

  let captured_unreachable = capture_events(&unreachable);
  assert_eq!(captured_unreachable.len(), 1);
  assert!(contains(&captured_unreachable[0], FIELD_TRANSITION, TRANSITION_DC_UNREACHABLE));
  assert!(contains(&captured_unreachable[0], FIELD_DATA_CENTER, "dc-east"));

  let captured_reachable = capture_events(&reachable);
  assert_eq!(captured_reachable.len(), 1);
  assert!(contains(&captured_reachable[0], FIELD_TRANSITION, TRANSITION_DC_REACHABLE));
  assert!(contains(&captured_reachable[0], FIELD_DATA_CENTER, "dc-east"));
}

#[test]
fn intermediate_status_changes_do_not_emit() {
  // shutdown 進行は専用イベント側で出力するため、status 変更経由では二重出力しない
  let cases = [
    NodeStatus::WeaklyUp,
    NodeStatus::Suspect,
    NodeStatus::Exiting,
    NodeStatus::PreparingForShutdown,
    NodeStatus::ReadyForShutdown,
    NodeStatus::Dead,
  ];
  for status in cases {
    let captured = capture_events(&member_status_changed(status));
    assert!(captured.is_empty(), "status {status:?} should not emit a lifecycle trace");
  }
}

#[test]
fn non_cluster_events_are_ignored() {
  let other_extension = EventStreamEvent::Extension {
    name:    String::from("other"),
    payload: AnyMessage::new(ClusterEvent::MemberPreparingForShutdown {
      node_id:     String::from("node-3"),
      authority:   String::from("127.0.0.1:7002"),
      observed_at: TimerInstant::from_ticks(4, Duration::from_secs(1)),
    }),
  };
  let non_cluster_payload =
    EventStreamEvent::Extension { name: String::from("cluster"), payload: AnyMessage::new(42_u32) };

  assert!(capture_events(&other_extension).is_empty());
  assert!(capture_events(&non_cluster_payload).is_empty());
}
