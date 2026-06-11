use alloc::{string::String, vec, vec::Vec};
use core::{slice::from_ref, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscription, subscriber_handle,
  },
  serialization::{default_serialization_setup, serialization_registry::SerializationRegistry},
};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

use super::ClusterPubSubImpl;
use crate::{
  ClusterEvent, ClusterTopology, TopologyUpdate,
  activation::ClusterIdentity,
  grain::KindRegistry,
  pub_sub::{
    DeliverBatchRequest, DeliveryEndpoint, DeliveryEndpointShared, DeliveryReport, DeliveryStatus,
    DistributedPubSubConfig, MediatorCommand, MediatorCommandOutcome, MediatorDeliveryIntent, MediatorDeliveryMode,
    MediatorPathKey, PubSubBatch, PubSubConfig, PubSubEnvelope, PubSubError, PubSubEvent, PubSubNoSubscriberBehavior,
    PubSubRoutingMode, PubSubSubscriber, PubSubTopic, PublishAck, PublishOptions, PublishRequest,
    SubscriberDeliveryReport, TopicRegistryApplyOutcome, TopicRegistryDelta, TopicRegistryDeltaEntry,
    TopicRegistryEntry, TopicRegistryEntryKey, TopicRegistryEntryKind, TopicRegistryStatus, TopicRegistryVersion,
    cluster_pub_sub::ClusterPubSub,
  },
};

/// EventStream イベントを収集するテスト用 subscriber
#[derive(Clone)]
struct TestSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl TestSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for TestSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe_recorder(event_stream: &EventStreamShared) -> (TestSubscriber, EventStreamSubscription) {
  let subscriber = TestSubscriber::new();
  let handle = subscriber_handle(subscriber.clone());
  let subscription = event_stream.subscribe(&handle);
  (subscriber, subscription)
}

fn extract_cluster_events(events: &[EventStreamEvent]) -> Vec<ClusterEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e
        && name == "cluster"
      {
        return payload.payload().downcast_ref::<ClusterEvent>().cloned();
      }
      None
    })
    .collect()
}

fn extract_pub_sub_events(events: &[EventStreamEvent]) -> Vec<PubSubEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e
        && name == "cluster-pubsub"
      {
        return payload.payload().downcast_ref::<PubSubEvent>().cloned();
      }
      None
    })
    .collect()
}

#[derive(Clone)]
struct StubEndpoint {
  failed: Vec<PubSubSubscriber>,
}

impl StubEndpoint {
  fn new(failed: Vec<PubSubSubscriber>) -> Self {
    Self { failed }
  }
}

impl DeliveryEndpoint for StubEndpoint {
  fn deliver(&mut self, request: DeliverBatchRequest) -> Result<DeliveryReport, PubSubError> {
    let failed = request
      .subscribers
      .into_iter()
      .filter(|subscriber| self.failed.contains(subscriber))
      .map(|subscriber| SubscriberDeliveryReport { subscriber, status: DeliveryStatus::SubscriberUnreachable })
      .collect();
    Ok(DeliveryReport { status: DeliveryStatus::Delivered, failed })
  }
}

fn make_pubsub(
  event_stream: EventStreamShared,
  registry: &KindRegistry,
  failed: Vec<PubSubSubscriber>,
) -> ClusterPubSubImpl {
  let setup = default_serialization_setup();
  let serialization_registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  let endpoint = DeliveryEndpointShared::new(Box::new(StubEndpoint::new(failed)));
  ClusterPubSubImpl::new(event_stream, serialization_registry, endpoint, PubSubConfig::default(), registry)
}

fn payload() -> PubSubEnvelope {
  PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() }
}

fn mediator_bucket_version(pubsub: &ClusterPubSubImpl, owner: &UniqueAddress) -> TopicRegistryVersion {
  pubsub
    .mediator_state
    .buckets()
    .into_iter()
    .find(|bucket| bucket.owner() == owner)
    .map_or(TopicRegistryVersion::zero(), |bucket| bucket.version())
}

fn mediator_bucket_has_entry(pubsub: &ClusterPubSubImpl, owner: &UniqueAddress, key: &TopicRegistryEntryKey) -> bool {
  pubsub
    .mediator_state
    .buckets()
    .into_iter()
    .find(|bucket| bucket.owner() == owner)
    .is_some_and(|bucket| bucket.entry(key).is_some())
}

#[test]
fn starts_when_topic_kind_is_registered() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (_subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let result = pubsub.start();
  assert!(result.is_ok(), "start should succeed when TopicActorKind is registered");
}

#[test]
fn fails_and_fires_event_when_topic_kind_missing() {
  let registry = KindRegistry::new();
  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let result = pubsub.start();
  assert!(result.is_err(), "start should fail when TopicActorKind is not registered");

  let collected = subscriber.events();
  let cluster_events = extract_cluster_events(&collected);
  assert!(
    cluster_events
      .iter()
      .any(|e| matches!(e, ClusterEvent::StartupFailed { reason, .. } if reason.contains("TopicActorKind"))),
    "should emit StartupFailed event with reason containing TopicActorKind"
  );
}

#[test]
fn publish_accepts_and_emits_events() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let subscriber_id = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  pubsub.subscribe(&topic, subscriber_id.clone()).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let ack = pubsub.publish(request).expect("publish");
  assert_eq!(ack, PublishAck::accepted());

  let events = extract_pub_sub_events(&subscriber.events());
  assert!(
    events.iter().any(|event| matches!(event, PubSubEvent::PublishAccepted { topic: t, .. } if t.as_str() == "news"))
  );
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::DeliverySucceeded { topic: t, subscriber } if t.as_str() == "news" && subscriber == "kind/sub-1")));
}

#[test]
fn publish_rejects_when_no_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let ack = pubsub.publish(request).expect("publish");
  assert_eq!(ack.status, crate::pub_sub::PublishStatus::Rejected);
}

#[test]
fn custom_mediator_config_are_exposed() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mediator_config = DistributedPubSubConfig::try_new(
    Some(String::from("backend")),
    PubSubRoutingMode::RoundRobin,
    Duration::from_secs(2),
    Duration::from_secs(60),
    64,
    PubSubNoSubscriberBehavior::DeadLetter,
  )
  .expect("settings");

  let pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_mediator_config(mediator_config.clone());

  assert_eq!(pubsub.mediator_config(), mediator_config);
}

#[test]
fn mediator_command_rebinds_owner_from_matching_active_membership_address() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_advertised_address("node-a:2552");
  pubsub.start().expect("start");

  let real_owner = UniqueAddress::new(Address::new("cluster", "node-a", 2552), 42);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "path-target").expect("identity"));
  let payload = PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() };

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&real_owner),
    )
    .expect("put");

  let sent = pubsub
    .apply_mediator_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload.clone(), false).expect("send"),
      11,
      from_ref(&real_owner),
    )
    .expect("send");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode: MediatorDeliveryMode::Send,
      targets: vec![target],
      payload,
    })
  );
}

#[test]
fn mediator_command_rebinds_owner_from_advertised_address_without_port() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_advertised_address("node-a");
  pubsub.start().expect("start");

  let real_owner = UniqueAddress::new(Address::new("cluster", "node-a", 2552), 42);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "path-target").expect("identity"));
  let payload = PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() };

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&real_owner),
    )
    .expect("put");

  let sent = pubsub
    .apply_mediator_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload.clone(), false).expect("send"),
      11,
      from_ref(&real_owner),
    )
    .expect("send");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode: MediatorDeliveryMode::Send,
      targets: vec![target],
      payload,
    })
  );
}

#[test]
fn mediator_command_rebinds_owner_from_bracketed_ipv6_advertised_address() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_advertised_address("[::1]:2552");
  pubsub.start().expect("start");

  let real_owner = UniqueAddress::new(Address::new("cluster", "::1", 2552), 42);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "path-target").expect("identity"));
  let payload = PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() };

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&real_owner),
    )
    .expect("put");

  let sent = pubsub
    .apply_mediator_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload.clone(), false).expect("send"),
      11,
      from_ref(&real_owner),
    )
    .expect("send");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode: MediatorDeliveryMode::Send,
      targets: vec![target],
      payload,
    })
  );
}

#[test]
fn mediator_command_rebinds_owner_from_bracketed_ipv6_advertised_address_without_port() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_advertised_address("[::1]");
  pubsub.start().expect("start");

  let real_owner = UniqueAddress::new(Address::new("cluster", "::1", 2552), 42);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "path-target").expect("identity"));
  let payload = PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() };

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&real_owner),
    )
    .expect("put");

  let sent = pubsub
    .apply_mediator_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload.clone(), false).expect("send"),
      11,
      from_ref(&real_owner),
    )
    .expect("send");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode: MediatorDeliveryMode::Send,
      targets: vec![target],
      payload,
    })
  );
}

#[test]
fn mediator_command_rebinds_owner_from_bracketed_ipv6_active_owner_host() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new()).with_advertised_address("[::1]:2552");
  pubsub.start().expect("start");

  let real_owner = UniqueAddress::new(Address::new("cluster", "[::1]", 2552), 42);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "path-target").expect("identity"));
  let payload = PubSubEnvelope { serializer_id: 41, type_name: String::from("dummy"), bytes: Vec::new() };

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put("fraktor://sys/user/service", target.clone()).expect("put"),
      10,
      from_ref(&real_owner),
    )
    .expect("put");

  let sent = pubsub
    .apply_mediator_command(
      MediatorCommand::try_send("fraktor://sys/user/service", payload.clone(), false).expect("send"),
      11,
      from_ref(&real_owner),
    )
    .expect("send");

  assert_eq!(
    sent,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode: MediatorDeliveryMode::Send,
      targets: vec![target],
      payload,
    })
  );
}

#[test]
fn topology_update_reactivates_suspended_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let topic = PubSubTopic::from("news");
  let subscriber_id = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  let mut pubsub = make_pubsub(event_stream.clone(), &registry, vec![subscriber_id.clone()]);
  pubsub.start().expect("start");
  pubsub.subscribe(&topic, subscriber_id.clone()).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let _ = pubsub.publish(request).expect("publish");

  let topology = ClusterTopology::new(1, vec![String::from("node-a")], Vec::new(), Vec::new());
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("node-a")],
    vec![String::from("node-a")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(2, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  let events = extract_pub_sub_events(&subscriber.events());
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::DeliveryFailed { topic: t, subscriber, .. }
    if t.as_str() == "news" && subscriber == "kind/sub-1")));
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::SubscriptionAdded { topic: t, subscriber }
    if t.as_str() == "news" && subscriber == "kind/sub-1")));
}

#[test]
fn topology_update_prunes_single_member_mediator_tombstones_after_ttl() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  let path = "fraktor://sys/user/service";
  let path_key = MediatorPathKey::parse(path).expect("path");
  let active_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 2552), 1);
  let command_now = 1_700_000_000_000;
  pubsub.start().expect("start");

  let topology = ClusterTopology::new(1, vec![String::from("pubsub")], Vec::new(), Vec::new());
  let initial_update = TopologyUpdate::new(
    topology.clone(),
    vec![String::from("pubsub")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
  );
  pubsub.on_topology(&initial_update);

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put(path, target.clone()).expect("put"),
      command_now,
      from_ref(&active_owner),
    )
    .expect("put command");
  pubsub
    .apply_mediator_command(
      MediatorCommand::try_remove(path, target.clone()).expect("remove"),
      command_now,
      from_ref(&active_owner),
    )
    .expect("remove command");
  let key = TopicRegistryEntryKey::Path { path: path_key, target };
  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_some());

  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(120, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_none());
}

#[test]
fn topology_update_prunes_local_only_mediator_tombstones_with_non_mediator_members() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  let path = "fraktor://sys/user/service";
  let path_key = MediatorPathKey::parse(path).expect("path");
  let active_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let command_now = 1_700_000_000_000;
  pubsub.start().expect("start");

  let topology = ClusterTopology::new(1, vec![String::from("pubsub"), String::from("worker")], Vec::new(), Vec::new());
  let initial_update = TopologyUpdate::new(
    topology.clone(),
    vec![String::from("pubsub"), String::from("worker")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
  );
  pubsub.on_topology(&initial_update);

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_put(path, target.clone()).expect("put"),
      command_now,
      from_ref(&active_owner),
    )
    .expect("put command");
  pubsub
    .apply_mediator_command(
      MediatorCommand::try_remove(path, target.clone()).expect("remove"),
      command_now,
      from_ref(&active_owner),
    )
    .expect("remove command");
  let key = TopicRegistryEntryKey::Path { path: path_key, target };
  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_some());

  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub"), String::from("worker")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(120, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_none());
}

#[test]
fn topology_update_prunes_multi_member_mediator_tombstones_after_peer_status_observes_them() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  let path = "fraktor://sys/user/service";
  let path_key = MediatorPathKey::parse(path).expect("path");
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let active_owners = vec![local_owner.clone(), remote_owner.clone()];
  let command_now = 1_700_000_000_000;
  pubsub.start().expect("start");

  let topology = ClusterTopology::new(
    1,
    vec![String::from("pubsub"), String::from("node-b"), String::from("worker")],
    Vec::new(),
    Vec::new(),
  );
  let initial_update = TopologyUpdate::new(
    topology.clone(),
    vec![String::from("pubsub"), String::from("node-b"), String::from("worker")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
  );
  pubsub.on_topology(&initial_update);

  pubsub
    .apply_mediator_command(MediatorCommand::try_put(path, target.clone()).expect("put"), command_now, &active_owners)
    .expect("put command");
  pubsub
    .apply_mediator_command(
      MediatorCommand::try_remove(path, target.clone()).expect("remove"),
      command_now,
      &active_owners,
    )
    .expect("remove command");
  let key = TopicRegistryEntryKey::Path { path: path_key, target };
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub"), String::from("node-b"), String::from("worker")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(120, Duration::from_secs(1)),
  );

  pubsub.on_topology(&update);
  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_some());

  pubsub.record_mediator_peer_status(
    remote_owner,
    TopicRegistryStatus::new(vec![(local_owner, TopicRegistryVersion::new(2))]),
  );
  pubsub.on_topology(&update);

  assert!(pubsub.mediator_state.local_bucket().entry(&key).is_none());
}

#[test]
fn mediator_delta_applies_remote_subscription_for_publish_delivery() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let topic = PubSubTopic::from("news");
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let active_owners = vec![local_owner, remote_owner.clone()];
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  };
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(remote_owner, key, entry)]);
  pubsub.start().expect("start");

  let outcomes = pubsub.apply_mediator_delta(&delta, &active_owners);
  let published = pubsub
    .apply_mediator_command(MediatorCommand::try_publish(topic, payload()).expect("publish"), 10, &active_owners)
    .expect("publish");

  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  assert_eq!(
    published,
    MediatorCommandOutcome::Delivery(MediatorDeliveryIntent::Deliver {
      mode:    MediatorDeliveryMode::Publish,
      targets: vec![subscriber],
      payload: payload(),
    })
  );
}

#[test]
fn mediator_gossip_status_reports_remote_observations_but_delta_excludes_remote_mirrors() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let active_owners = vec![local_owner.clone(), remote_owner.clone()];
  let local_topic = PubSubTopic::from("local-news");
  let remote_topic = PubSubTopic::from("remote-news");
  let local_subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "local").expect("identity"));
  let remote_subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let remote_key = TopicRegistryEntryKey::TopicSubscription {
    topic:      remote_topic.clone(),
    group:      None,
    subscriber: remote_subscriber.clone(),
  };
  let remote_entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic:      remote_topic,
    group:      None,
    subscriber: remote_subscriber,
  });
  let delta =
    TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(remote_owner.clone(), remote_key, remote_entry)]);
  pubsub.start().expect("start");

  pubsub
    .apply_mediator_command(
      MediatorCommand::try_subscribe(local_topic, None, local_subscriber).expect("subscribe"),
      10,
      &active_owners,
    )
    .expect("subscribe");
  pubsub.apply_mediator_delta(&delta, &active_owners);

  let status = pubsub.mediator_status();
  let gossip_delta = pubsub.collect_mediator_delta(&TopicRegistryStatus::default());

  assert!(status.version_for(&local_owner).value() > 0);
  assert!(status.version_for(&remote_owner).value() > 0);
  assert!(gossip_delta.entries().iter().all(|entry| entry.owner() == &local_owner));
}

#[test]
fn topology_update_evicts_inactive_remote_bucket_from_mediator_status() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let topic = PubSubTopic::from("news");
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  };
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic,
    group: None,
    subscriber,
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(remote_owner.clone(), key, entry)]);
  let active_owners = vec![UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1), remote_owner.clone()];
  pubsub.start().expect("start");
  pubsub.apply_mediator_delta(&delta, &active_owners);
  assert!(mediator_bucket_version(&pubsub, &remote_owner).value() > 0);

  let topology = ClusterTopology::new(1, vec![String::from("pubsub")], Vec::new(), Vec::new());
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(1, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert_eq!(mediator_bucket_version(&pubsub, &remote_owner), TopicRegistryVersion::zero());
}

#[test]
fn topology_update_evicts_remote_bucket_when_active_owner_uid_changes() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let old_remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let new_remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 3);
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let topic = PubSubTopic::from("news");
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  };
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic: topic.clone(),
    group: None,
    subscriber,
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(old_remote_owner.clone(), key, entry)]);
  pubsub.start().expect("start");
  pubsub.apply_mediator_delta(&delta, &[local_owner.clone(), old_remote_owner.clone()]);
  pubsub.record_mediator_peer_status(
    old_remote_owner.clone(),
    TopicRegistryStatus::new(vec![(local_owner.clone(), TopicRegistryVersion::new(1))]),
  );
  assert!(mediator_bucket_version(&pubsub, &old_remote_owner).value() > 0);
  assert!(pubsub.peer_statuses.contains_key(&old_remote_owner));

  pubsub
    .apply_mediator_command(MediatorCommand::subscriber_count(topic).expect("query"), 10, &[
      local_owner,
      new_remote_owner,
    ])
    .expect("query");
  let topology = ClusterTopology::new(1, vec![String::from("pubsub"), String::from("node-b")], Vec::new(), Vec::new());
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub"), String::from("node-b")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(1, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert_eq!(mediator_bucket_version(&pubsub, &old_remote_owner), TopicRegistryVersion::zero());
  assert!(!pubsub.peer_statuses.contains_key(&old_remote_owner));
}

#[test]
fn topology_update_invalidates_stale_active_owner_cache_for_dead_member() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let old_remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let topic = PubSubTopic::from("news");
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  };
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::TopicSubscription {
    topic,
    group: None,
    subscriber,
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(old_remote_owner.clone(), key, entry)]);
  pubsub.start().expect("start");
  pubsub.apply_mediator_delta(&delta, &[local_owner.clone(), old_remote_owner.clone()]);
  pubsub.record_mediator_peer_status(
    old_remote_owner.clone(),
    TopicRegistryStatus::new(vec![(local_owner, TopicRegistryVersion::new(1))]),
  );
  assert!(mediator_bucket_version(&pubsub, &old_remote_owner).value() > 0);
  assert!(pubsub.peer_statuses.contains_key(&old_remote_owner));

  let topology = ClusterTopology::new(1, vec![String::from("pubsub"), String::from("node-b")], Vec::new(), Vec::new());
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub"), String::from("node-b")],
    Vec::new(),
    Vec::new(),
    vec![String::from("node-b")],
    Vec::new(),
    TimerInstant::from_ticks(1, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert_eq!(mediator_bucket_version(&pubsub, &old_remote_owner), TopicRegistryVersion::zero());
  assert!(!pubsub.peer_statuses.contains_key(&old_remote_owner));
}

#[test]
fn topology_update_prunes_delta_tombstones_without_local_mediator_command() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let local_owner = UniqueAddress::new(Address::new("fraktor-cluster", "pubsub", 0), 1);
  let remote_owner = UniqueAddress::new(Address::new("fraktor-cluster", "node-b", 0), 2);
  let target = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "remote").expect("identity"));
  let path = MediatorPathKey::parse("fraktor://sys/user/service").expect("path");
  let key = TopicRegistryEntryKey::Path { path, target };
  let command_now = 1_700_000_000_000;
  let entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Removed {
    removed_at_millis: command_now,
  });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(remote_owner.clone(), key.clone(), entry)]);
  let active_owners = vec![local_owner, remote_owner.clone()];
  pubsub.start().expect("start");

  let topology = ClusterTopology::new(1, vec![String::from("pubsub"), String::from("node-b")], Vec::new(), Vec::new());
  let initial_update = TopologyUpdate::new(
    topology.clone(),
    vec![String::from("pubsub"), String::from("node-b")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
  );
  pubsub.on_topology(&initial_update);

  let outcomes = pubsub.apply_mediator_delta(&delta, &active_owners);
  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  pubsub.record_mediator_peer_status(
    remote_owner.clone(),
    TopicRegistryStatus::new(vec![(remote_owner.clone(), TopicRegistryVersion::new(1))]),
  );
  assert!(mediator_bucket_has_entry(&pubsub, &remote_owner, &key));

  let update = TopologyUpdate::new(
    topology,
    vec![String::from("pubsub"), String::from("node-b")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(120, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  assert!(!mediator_bucket_has_entry(&pubsub, &remote_owner, &key));
}
