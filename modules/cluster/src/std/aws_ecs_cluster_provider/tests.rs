//! Tests for AwsEcsClusterProvider.

use std::sync::Mutex;

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, subscriber_handle,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{AwsEcsClusterProvider, EcsClusterConfig};
use crate::core::{ClusterEvent, StartupMode, cluster_provider::ClusterProvider};

struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<Mutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(Mutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().unwrap().clone()
  }
}

impl EventStreamSubscriber<StdToolbox> for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.events.lock().unwrap().push(cluster_event.clone());
        }
      }
    }
  }
}

fn subscribe_recorder(
  event_stream: &EventStreamSharedGeneric<StdToolbox>,
) -> (RecordingClusterEvents, EventStreamSubscriptionGeneric<StdToolbox>) {
  let subscriber_impl = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(subscriber_impl.clone());
  let subscription = event_stream.subscribe(&subscriber);
  (subscriber_impl, subscription)
}

#[test]
fn ecs_cluster_config_default_values() {
  let config = EcsClusterConfig::new();
  assert_eq!(config.cluster_name(), "");
  assert!(config.service_name().is_none());
  assert_eq!(config.poll_interval().as_secs(), 30);
  assert_eq!(config.port(), 8080);
  assert!(config.region().is_none());
}

#[test]
fn ecs_cluster_config_builder_pattern() {
  use std::time::Duration;

  let config = EcsClusterConfig::new()
    .with_cluster_name("my-cluster")
    .with_service_name("my-service")
    .with_poll_interval(Duration::from_secs(10))
    .with_port(9090)
    .with_region("ap-northeast-1");

  assert_eq!(config.cluster_name(), "my-cluster");
  assert_eq!(config.service_name(), Some("my-service"));
  assert_eq!(config.poll_interval().as_secs(), 10);
  assert_eq!(config.port(), 9090);
  assert_eq!(config.region(), Some("ap-northeast-1"));
}

#[test]
fn provider_new_creates_instance() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  assert_eq!(provider.advertised_address(), "127.0.0.1:8080");
  assert!(!provider.is_started());
  assert_eq!(provider.member_count(), 0);
}

#[test]
fn provider_with_ecs_config() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let ecs_config = EcsClusterConfig::new().with_cluster_name("test-cluster").with_service_name("test-service");

  let provider =
    AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080").with_ecs_config(ecs_config.clone());

  assert_eq!(provider.advertised_address(), "127.0.0.1:8080");
}

#[tokio::test]
async fn start_member_publishes_startup_event() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let result = provider.start_member();
  assert!(result.is_ok());
  assert!(provider.is_started());

  let events = subscriber_impl.events();
  // TopologyUpdated と Startup の 2 イベント
  assert!(events.len() >= 2);

  // Startup イベントを確認
  let startup_event = events.iter().find(|e| matches!(e, ClusterEvent::Startup { .. }));
  assert!(startup_event.is_some());
  if let Some(ClusterEvent::Startup { address, mode }) = startup_event {
    assert_eq!(address, "127.0.0.1:8080");
    assert_eq!(*mode, StartupMode::Member);
  }

  let _ = provider.shutdown(true);
}

#[tokio::test]
async fn start_client_publishes_startup_event() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let result = provider.start_client();
  assert!(result.is_ok());
  assert!(provider.is_started());

  let events = subscriber_impl.events();
  assert!(!events.is_empty());

  // Startup イベントを確認
  let startup_event = events.iter().find(|e| matches!(e, ClusterEvent::Startup { .. }));
  assert!(startup_event.is_some());
  if let Some(ClusterEvent::Startup { address, mode }) = startup_event {
    assert_eq!(address, "127.0.0.1:8080");
    assert_eq!(*mode, StartupMode::Client);
  }

  let _ = provider.shutdown(true);
}

#[tokio::test]
async fn shutdown_publishes_shutdown_event() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let _ = provider.start_member();
  let result = provider.shutdown(true);

  assert!(result.is_ok());
  assert!(!provider.is_started());

  let events = subscriber_impl.events();
  let shutdown_event = events.iter().find(|e| matches!(e, ClusterEvent::Shutdown { .. }));
  assert!(shutdown_event.is_some());
  if let Some(ClusterEvent::Shutdown { address, mode }) = shutdown_event {
    assert_eq!(address, "127.0.0.1:8080");
    assert_eq!(*mode, StartupMode::Member);
  }
}

#[test]
fn down_rejects_self_authority() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);
  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let result = provider.down("127.0.0.1:8080");

  assert!(
    matches!(result, Err(crate::core::ClusterProviderError::DownFailed(reason)) if reason == "cannot down self authority")
  );
}

#[test]
fn down_unknown_node_is_noop() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);
  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");
  provider.members = vec![String::from("127.0.0.1:8080"), String::from("127.0.0.1:8081")];

  let result = provider.down("127.0.0.1:9090");

  assert!(result.is_ok());
  assert_eq!(provider.member_count(), 2);
  assert!(subscriber_impl.events().is_empty());
}

#[test]
fn down_known_node_removes_member_and_publishes_left() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);
  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");
  provider.members = vec![String::from("127.0.0.1:8080"), String::from("127.0.0.1:8081")];

  let result = provider.down("127.0.0.1:8081");

  assert!(result.is_ok());
  assert_eq!(provider.member_count(), 1);
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(
    event,
    ClusterEvent::TopologyUpdated { update } if update.left == vec![String::from("127.0.0.1:8081")]
  )));
}

#[test]
fn join_is_explicitly_unsupported() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);
  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let result = provider.join("127.0.0.1:9090");

  assert!(
    matches!(result, Err(crate::core::ClusterProviderError::JoinFailed(reason)) if reason == "join is not supported by aws ecs provider")
  );
}

#[test]
fn leave_is_explicitly_unsupported() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);
  let mut provider = AwsEcsClusterProvider::new(event_stream, block_list, "127.0.0.1:8080");

  let result = provider.leave("127.0.0.1:9090");

  assert!(
    matches!(result, Err(crate::core::ClusterProviderError::LeaveFailed(reason)) if reason == "leave is not supported by aws ecs provider")
  );
}
