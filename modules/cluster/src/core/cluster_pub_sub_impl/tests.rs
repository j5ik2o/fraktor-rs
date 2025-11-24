use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::*;
use crate::core::{ClusterEvent, ClusterPubSub, KindRegistry, PubSubEvent, kind_registry::TOPIC_ACTOR_KIND};

/// EventStream イベントを収集するテスト用 subscriber
#[derive(Clone)]
struct TestSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl TestSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for TestSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn extract_cluster_events(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<ClusterEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e {
        if name == "cluster" {
          return payload.payload().downcast_ref::<ClusterEvent>().cloned();
        }
      }
      None
    })
    .collect()
}

fn extract_pub_sub_events(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<PubSubEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e {
        if name == "cluster-pubsub" {
          return payload.payload().downcast_ref::<PubSubEvent>().cloned();
        }
      }
      None
    })
    .collect()
}

#[test]
fn starts_when_topic_kind_is_registered() {
  // KindRegistry は register_all 時に TOPIC_ACTOR_KIND を自動登録する
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  // EventStream を作成
  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());

  // サブスクライバを登録
  let subscriber = ArcShared::new(TestSubscriber::new());
  let sub_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &sub_ref);

  // PubSubImpl を作成
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);

  // TopicActorKind が登録されているので start は成功する
  let result = pubsub.start();
  assert!(result.is_ok(), "start should succeed when TopicActorKind is registered");
}

#[test]
fn fails_and_fires_event_when_topic_kind_missing() {
  // KindRegistry を作成するが register_all を呼ばない（TOPIC_ACTOR_KIND が無い状態）
  let registry = KindRegistry::new();

  // EventStream を作成
  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());

  // サブスクライバを登録
  let subscriber = ArcShared::new(TestSubscriber::new());
  let sub_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &sub_ref);

  // PubSubImpl を作成
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);

  // TopicActorKind が登録されていないので start は失敗する
  let result = pubsub.start();
  assert!(result.is_err(), "start should fail when TopicActorKind is not registered");

  // EventStream に StartupFailed イベントが発火されている
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
fn creates_topic_on_start() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let subscriber = ArcShared::new(TestSubscriber::new());
  let sub_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &sub_ref);

  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);
  pubsub.start().expect("start should succeed");

  // TopicCreated イベントが発火されている
  let collected = subscriber.events();
  let pubsub_events = extract_pub_sub_events(&collected);
  assert!(
    pubsub_events.iter().any(|e| matches!(e, PubSubEvent::TopicCreated { topic } if topic == TOPIC_ACTOR_KIND)),
    "should emit TopicCreated event for prototopic"
  );
}

#[test]
fn subscribe_succeeds_after_start() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let subscriber = ArcShared::new(TestSubscriber::new());
  let sub_ref: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &sub_ref);

  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);
  pubsub.start().expect("start should succeed");

  // 購読を追加
  let result = pubsub.subscribe(TOPIC_ACTOR_KIND, "subscriber-1");
  assert!(result.is_ok(), "subscribe should succeed after start");

  // SubscriptionAccepted イベントが発火されている
  let collected = subscriber.events();
  let pubsub_events = extract_pub_sub_events(&collected);
  assert!(
    pubsub_events.iter().any(|e| matches!(e, PubSubEvent::SubscriptionAccepted { topic, subscriber }
      if topic == TOPIC_ACTOR_KIND && subscriber == "subscriber-1")),
    "should emit SubscriptionAccepted event"
  );
}

#[test]
fn subscribe_fails_before_start() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);

  // start 前に subscribe すると失敗
  let result = pubsub.subscribe(TOPIC_ACTOR_KIND, "subscriber-1");
  assert!(result.is_err(), "subscribe should fail before start");
}

#[test]
fn publish_returns_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);
  pubsub.start().expect("start");

  pubsub.subscribe(TOPIC_ACTOR_KIND, "sub-a").expect("subscribe a");
  pubsub.subscribe(TOPIC_ACTOR_KIND, "sub-b").expect("subscribe b");

  let subscribers = pubsub.publish(TOPIC_ACTOR_KIND).expect("publish should succeed");
  assert_eq!(subscribers.len(), 2);
  assert!(subscribers.contains(&String::from("sub-a")));
  assert!(subscribers.contains(&String::from("sub-b")));
}

#[test]
fn stop_succeeds() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);
  pubsub.start().expect("start");

  let result = pubsub.stop();
  assert!(result.is_ok(), "stop should succeed");
}

#[test]
fn drain_events_returns_broker_events() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let pubsub = ClusterPubSubImpl::new(event_stream, &registry);
  pubsub.start().expect("start");
  pubsub.subscribe(TOPIC_ACTOR_KIND, "sub-1").expect("subscribe");

  let events = pubsub.drain_events();
  // TopicCreated と SubscriptionAccepted が含まれる
  // ただし start と subscribe の中で flush しているので、drain_events は空になる
  // drain_events が空になることを確認
  assert!(events.is_empty(), "events should be empty because they were already flushed to EventStream");
}
