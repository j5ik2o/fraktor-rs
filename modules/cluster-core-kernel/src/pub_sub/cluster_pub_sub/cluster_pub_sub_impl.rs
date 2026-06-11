//! EventStream-based ClusterPubSub implementation backed by PubSubBroker.

#[cfg(test)]
#[path = "cluster_pub_sub_impl_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec};
use core::{slice::from_ref, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
  serialization::{SerializationError, serialization_registry::SerializationRegistry},
};
use fraktor_remote_core_rs::address::{Address, UniqueAddress};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};

use super::ClusterPubSub;
use crate::{
  ClusterEvent, StartupMode, TopologyUpdate,
  grain::{KindRegistry, TOPIC_ACTOR_KIND},
  pub_sub::{
    DeliverBatchRequest, DeliveryEndpointShared, DeliveryReport, DistributedPubSubConfig,
    DistributedPubSubMediatorState, MediatorCommand, MediatorCommandOutcome, PubSubBatch, PubSubBroker, PubSubConfig,
    PubSubEnvelope, PubSubError, PubSubEvent, PubSubSubscriber, PubSubTopic, PubSubTopicOptions, PublishAck,
    PublishOptions, PublishRejectReason, PublishRequest, SubscriberDeliveryReport, TopicRegistryApplyOutcome,
    TopicRegistryDelta, TopicRegistryDeltaCollector, TopicRegistryEntryKind, TopicRegistryStatus,
  },
};

/// PubSubBroker-backed ClusterPubSub implementation with EventStream integration.
///
/// This implementation requires TopicActorKind to be registered in the KindRegistry
/// before starting. On start, it creates the topic for TopicActorKind and publishes
/// events to EventStream.
pub struct ClusterPubSubImpl {
  event_stream:           EventStreamShared,
  broker:                 PubSubBroker,
  has_topic_actor_kind:   bool,
  started:                bool,
  advertised_address:     String,
  pubsub_config:          PubSubConfig,
  delivery_endpoint:      DeliveryEndpointShared,
  registry:               ArcShared<SerializationRegistry>,
  last_observed_at:       Option<TimerInstant>,
  mediator_clock_anchor:  Option<MediatorClockAnchor>,
  last_mediator_now:      Option<u64>,
  mediator_state:         DistributedPubSubMediatorState,
  peer_statuses:          BTreeMap<UniqueAddress, TopicRegistryStatus>,
  active_mediator_owners: Vec<UniqueAddress>,
}

#[derive(Debug, Clone, Copy)]
struct MediatorClockAnchor {
  observed_at: TimerInstant,
  now_millis:  u64,
}

impl ClusterPubSubImpl {
  /// Creates a new PubSubImpl with EventStream and KindRegistry reference.
  ///
  /// The KindRegistry is checked for TopicActorKind presence at construction time.
  #[must_use]
  pub fn new(
    event_stream: EventStreamShared,
    registry: ArcShared<SerializationRegistry>,
    delivery_endpoint: DeliveryEndpointShared,
    config: PubSubConfig,
    registry_snapshot: &KindRegistry,
  ) -> Self {
    let has_topic_actor_kind = registry_snapshot.contains(TOPIC_ACTOR_KIND);
    let mediator_config = DistributedPubSubConfig::default();
    let mediator_owner = mediator_owner_from_address("pubsub");
    Self {
      event_stream,
      broker: PubSubBroker::new(),
      has_topic_actor_kind,
      started: false,
      advertised_address: String::from("pubsub"),
      pubsub_config: config,
      delivery_endpoint,
      registry,
      last_observed_at: None,
      mediator_clock_anchor: None,
      last_mediator_now: None,
      mediator_state: DistributedPubSubMediatorState::new(mediator_config, mediator_owner),
      peer_statuses: BTreeMap::new(),
      active_mediator_owners: Vec::new(),
    }
  }

  /// Creates a new PubSubImpl with a custom advertised address.
  #[must_use]
  pub fn with_advertised_address(mut self, address: impl Into<String>) -> Self {
    self.advertised_address = address.into();
    self.mediator_state.rebind_local_owner(mediator_owner_from_address(&self.advertised_address));
    self
  }

  /// Creates a new PubSubImpl with custom distributed mediator configuration.
  #[must_use]
  pub fn with_mediator_config(mut self, settings: DistributedPubSubConfig) -> Self {
    let local_owner = self.mediator_state.local_owner().clone();
    self.mediator_state = DistributedPubSubMediatorState::new(settings, local_owner);
    self
  }

  /// Drains broker events (for testing).
  #[must_use]
  pub fn drain_events(&mut self) -> Vec<PubSubEvent> {
    self.broker.drain_events()
  }

  fn flush_broker_events_to_stream(&mut self) {
    let events = self.broker.drain_events();
    for event in events {
      self.publish_pubsub_event(event);
    }
  }

  fn publish_pubsub_event(&self, event: PubSubEvent) {
    let payload = AnyMessage::new(event);
    let stream_event = EventStreamEvent::Extension { name: String::from("cluster-pubsub"), payload };
    self.event_stream.publish(&stream_event);
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessage::new(event);
    let stream_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&stream_event);
  }

  fn effective_options(&mut self, topic: &PubSubTopic, overrides: PublishOptions) -> Option<PubSubTopicOptions> {
    let defaults = self.broker.topic_options(topic).ok()?;
    Some(defaults.apply_overrides(&overrides))
  }

  fn serialize_payload(&self, payload: &AnyMessage) -> Result<PubSubBatch, SerializationError> {
    let payload_any = payload.payload();
    let type_id = payload_any.type_id();
    let type_name =
      self.registry.binding_name(type_id).unwrap_or_else(|| String::from(core::any::type_name_of_val(payload_any)));
    let (serializer, _) = self.registry.serializer_for_type(type_id, &type_name, None)?;
    let bytes = serializer.to_binary(payload_any)?;
    let mut name = type_name;
    if let Some(provider) = serializer.as_string_manifest() {
      name = provider.manifest(payload_any).into_owned();
    }
    let envelope = PubSubEnvelope { serializer_id: serializer.identifier().value(), type_name: name, bytes };
    Ok(PubSubBatch::new(vec![envelope]))
  }

  fn map_serialization_error(error: &SerializationError) -> Result<PublishAck, PubSubError> {
    if error.is_not_serializable() {
      return Ok(PublishAck::rejected(PublishRejectReason::NotSerializable));
    }
    Err(PubSubError::SerializationFailed { reason: format!("{error:?}") })
  }

  fn split_subscribers(subscribers: Vec<PubSubSubscriber>) -> (Vec<PubSubSubscriber>, Vec<PubSubSubscriber>) {
    let mut local = Vec::new();
    let mut remote = Vec::new();
    for subscriber in subscribers {
      match subscriber {
        | PubSubSubscriber::ActorRef(_) => local.push(subscriber),
        | PubSubSubscriber::ClusterIdentity(_) => remote.push(subscriber),
      }
    }
    (local, remote)
  }

  fn rebind_mediator_owner_from_active_owners(&mut self, active_owners: &[UniqueAddress]) {
    // Empty active owners means membership has not supplied an observation yet; delivery views still
    // receive the same empty slice and expose no active buckets.
    if let Some(owner) = mediator_owner_from_active_owners(&self.advertised_address, active_owners) {
      self.mediator_state.rebind_local_owner(owner);
      self.peer_statuses.remove(self.mediator_state.local_owner());
    }
  }

  fn record_active_mediator_owners(&mut self, active_owners: &[UniqueAddress]) {
    self.active_mediator_owners = active_owners.to_vec();
    if self.active_mediator_owners.is_empty() {
      return;
    }

    let local_owner = self.mediator_state.local_owner().clone();
    self
      .peer_statuses
      .retain(|owner, _| owner != &local_owner && self.active_mediator_owners.iter().any(|active| active == owner));
  }

  fn refresh_active_mediator_owners_from_topology(&mut self, update: &TopologyUpdate) {
    if self.active_mediator_owners.is_empty() {
      return;
    }

    let local_owner = self.mediator_state.local_owner().clone();
    let advertised_address = self.advertised_address.clone();
    self.active_mediator_owners.retain(|owner| {
      owner == &local_owner
        || owner_matches_member(owner, &advertised_address)
        || !owner_unavailable_in_topology(owner, update)
    });
    self.peer_statuses.retain(|owner, _| !owner_unavailable_in_topology(owner, update));
  }

  fn active_peer_statuses(&self, update: &TopologyUpdate) -> Option<Vec<TopicRegistryStatus>> {
    let has_active_mediator_peer = self.has_active_mediator_peer(update);
    let mut statuses = Vec::new();
    for (owner, status) in &self.peer_statuses {
      if owner != self.mediator_state.local_owner()
        && owner_is_active_or_topology_member(owner, &self.active_mediator_owners, &update.members)
      {
        statuses.push(status.clone());
      }
    }
    if statuses.is_empty() && has_active_mediator_peer { None } else { Some(statuses) }
  }

  fn has_active_mediator_peer(&self, update: &TopologyUpdate) -> bool {
    if self.active_mediator_owners.is_empty() {
      return update.members.len() > 1;
    }

    self.active_mediator_owners.iter().any(|owner| {
      !self.is_local_mediator_owner(owner) && update.members.iter().any(|member| owner_matches_member(owner, member))
    })
  }

  fn is_local_mediator_owner(&self, owner: &UniqueAddress) -> bool {
    owner == self.mediator_state.local_owner() || owner_matches_member(owner, &self.advertised_address)
  }

  const fn record_mediator_now(&mut self, now_millis: u64) {
    self.last_mediator_now = Some(now_millis);
    if let Some(observed_at) = self.last_observed_at {
      self.mediator_clock_anchor = Some(MediatorClockAnchor { observed_at, now_millis });
    }
  }

  fn record_mediator_delta_clock(&mut self, delta: &TopicRegistryDelta) {
    if self.last_mediator_now.is_some() {
      return;
    }

    let Some(now_millis) = delta
      .entries()
      .iter()
      .filter_map(|entry| match entry.entry().kind() {
        | TopicRegistryEntryKind::Removed { removed_at_millis } => Some(*removed_at_millis),
        | TopicRegistryEntryKind::Path { .. } | TopicRegistryEntryKind::TopicSubscription { .. } => None,
      })
      .max()
    else {
      return;
    };
    self.record_mediator_now(now_millis);
  }

  fn mediator_now_for_topology(&mut self, observed_at: TimerInstant) -> Option<u64> {
    if let Some(anchor) = self.mediator_clock_anchor
      && let Some(elapsed_millis) = elapsed_millis(anchor.observed_at, observed_at)
    {
      return Some(anchor.now_millis.saturating_add(elapsed_millis));
    }

    let now_millis = self.last_mediator_now?;
    self.mediator_clock_anchor = Some(MediatorClockAnchor { observed_at, now_millis });
    Some(now_millis)
  }

  fn deliver_group(
    &mut self,
    topic: &PubSubTopic,
    batch: PubSubBatch,
    subscribers: &[PubSubSubscriber],
    options: PubSubTopicOptions,
  ) -> Result<(), PubSubError> {
    let deliver_request =
      DeliverBatchRequest { topic: topic.clone(), batch, subscribers: subscribers.to_vec(), options };
    let report = self.delivery_endpoint.with_write(|endpoint| endpoint.deliver(deliver_request));
    match report {
      | Ok(report) => {
        self.handle_delivery_report(topic, subscribers, report);
        Ok(())
      },
      | Err(error) => {
        self.flush_broker_events_to_stream();
        Err(error)
      },
    }
  }

  fn handle_delivery_report(&mut self, topic: &PubSubTopic, subscribers: &[PubSubSubscriber], report: DeliveryReport) {
    let now = self.last_observed_at.unwrap_or_else(|| TimerInstant::from_ticks(0, Duration::from_secs(1)));

    let mut failed_subscribers = Vec::new();
    for SubscriberDeliveryReport { subscriber, status } in report.failed {
      failed_subscribers.push(subscriber.clone());
      drop(self.broker.suspend_subscriber(topic, &subscriber, format!("{status:?}"), now));
      self.publish_pubsub_event(PubSubEvent::DeliveryFailed {
        topic: topic.clone(),
        subscriber: subscriber.label(),
        status,
      });
    }

    for subscriber in subscribers {
      if !failed_subscribers.contains(subscriber) {
        self.publish_pubsub_event(PubSubEvent::DeliverySucceeded {
          topic:      topic.clone(),
          subscriber: subscriber.label(),
        });
      }
    }
  }
}

impl ClusterPubSub for ClusterPubSubImpl {
  fn start(&mut self) -> Result<(), PubSubError> {
    // TopicActorKind がなければ起動失敗
    if !self.has_topic_actor_kind {
      let reason = format!("TopicActorKind '{}' is not registered in KindRegistry", TOPIC_ACTOR_KIND);
      self.publish_cluster_event(ClusterEvent::StartupFailed {
        address: self.advertised_address.clone(),
        mode:    StartupMode::Member,
        reason:  reason.clone(),
      });
      return Err(PubSubError::TopicNotFound { topic: PubSubTopic::from(reason) });
    }

    // prototopic トピックを作成
    let result = self.broker.create_topic(PubSubTopic::from(TOPIC_ACTOR_KIND));
    self.flush_broker_events_to_stream();

    // 重複時はエラーだが起動は成功とみなす
    match result {
      | Ok(()) | Err(PubSubError::TopicAlreadyExists { .. }) => {
        self.started = true;
        Ok(())
      },
      | Err(e) => {
        self.publish_cluster_event(ClusterEvent::StartupFailed {
          address: self.advertised_address.clone(),
          mode:    StartupMode::Member,
          reason:  format!("{e:?}"),
        });
        Err(e)
      },
    }
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    self.started = false;
    Ok(())
  }

  fn subscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    let result = self.broker.subscribe(topic, &subscriber);
    self.flush_broker_events_to_stream();
    result
  }

  fn unsubscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    let result = self.broker.unsubscribe(topic, &subscriber);
    self.flush_broker_events_to_stream();
    result
  }

  fn publish(&mut self, request: PublishRequest) -> Result<PublishAck, PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    if request.topic.is_empty() {
      return Ok(PublishAck::rejected(PublishRejectReason::InvalidTopic));
    }

    let Some(options) = self.effective_options(&request.topic, request.options) else {
      self.flush_broker_events_to_stream();
      return Ok(PublishAck::rejected(PublishRejectReason::InvalidTopic));
    };

    let batch = if let Some(batch) = request.payload.payload().downcast_ref::<PubSubBatch>() {
      if batch.is_empty() {
        return Ok(PublishAck::rejected(PublishRejectReason::InvalidPayload));
      }
      batch.clone()
    } else {
      match self.serialize_payload(&request.payload) {
        | Ok(batch) => batch,
        | Err(error) => return Self::map_serialization_error(&error),
      }
    };

    let subscribers = match self.broker.publish_targets(&request.topic, options) {
      | Ok(subscribers) => subscribers,
      | Err(reason) => {
        self.flush_broker_events_to_stream();
        return Ok(PublishAck::rejected(reason));
      },
    };

    if !subscribers.is_empty() {
      let (local_subscribers, remote_subscribers) = Self::split_subscribers(subscribers);
      match (local_subscribers.is_empty(), remote_subscribers.is_empty()) {
        | (false, false) => {
          let local_batch = batch.clone();
          self.deliver_group(&request.topic, local_batch, &local_subscribers, options)?;
          self.deliver_group(&request.topic, batch, &remote_subscribers, options)?;
        },
        | (false, true) => {
          self.deliver_group(&request.topic, batch, &local_subscribers, options)?;
        },
        | (true, false) => {
          self.deliver_group(&request.topic, batch, &remote_subscribers, options)?;
        },
        | (true, true) => {},
      }
    }

    self.flush_broker_events_to_stream();
    Ok(PublishAck::accepted())
  }

  fn mediator_config(&self) -> DistributedPubSubConfig {
    self.mediator_state.settings().clone()
  }

  fn mediator_status(&self) -> TopicRegistryStatus {
    TopicRegistryStatus::from_buckets(&self.mediator_state.buckets())
  }

  fn record_mediator_peer_status(&mut self, owner: UniqueAddress, status: TopicRegistryStatus) {
    if owner != *self.mediator_state.local_owner() {
      self.peer_statuses.insert(owner, status);
    }
  }

  fn collect_mediator_delta(&self, peer_status: &TopicRegistryStatus) -> TopicRegistryDelta {
    TopicRegistryDeltaCollector::collect_delta(
      peer_status,
      from_ref(self.mediator_state.local_bucket()),
      self.mediator_state.settings(),
    )
  }

  fn apply_mediator_delta(
    &mut self,
    delta: &TopicRegistryDelta,
    active_owners: &[UniqueAddress],
  ) -> Vec<TopicRegistryApplyOutcome> {
    self.rebind_mediator_owner_from_active_owners(active_owners);
    self.record_active_mediator_owners(active_owners);
    self.record_mediator_delta_clock(delta);
    self.mediator_state.apply_delta(delta, active_owners)
  }

  fn apply_mediator_command(
    &mut self,
    command: MediatorCommand,
    now_millis: u64,
    active_owners: &[UniqueAddress],
  ) -> Result<MediatorCommandOutcome, PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    self.rebind_mediator_owner_from_active_owners(active_owners);
    self.record_active_mediator_owners(active_owners);
    self.record_mediator_now(now_millis);
    self.mediator_state.apply_command(command, now_millis, active_owners)
  }

  fn on_topology(&mut self, update: &TopologyUpdate) {
    let mediator_now = self.mediator_now_for_topology(update.observed_at);
    self.refresh_active_mediator_owners_from_topology(update);
    let active_mediator_owners = self.active_mediator_owners.clone();
    let topology_members = update.members.clone();
    self.last_observed_at = Some(update.observed_at);
    self.mediator_state.retain_remote_buckets_by_owner(|owner| {
      owner_is_active_or_topology_member(owner, &active_mediator_owners, &topology_members)
    });
    if let Some(now_millis) = mediator_now
      && let Some(peer_statuses) = self.active_peer_statuses(update)
    {
      self.mediator_state.prune_removed_entries(now_millis, &peer_statuses);
    }
    for topic in self.broker.topics() {
      if let Ok(removed) =
        self.broker.remove_expired_suspended(&topic, update.observed_at, self.pubsub_config.suspended_ttl)
      {
        for subscriber in removed {
          self.publish_pubsub_event(PubSubEvent::SubscriptionRemoved {
            topic:      topic.clone(),
            subscriber: subscriber.label(),
            reason:     String::from("suspended_ttl_expired"),
          });
        }
      }

      if let Ok(reactivated) = self.broker.reactivate_all(&topic) {
        for subscriber in reactivated {
          self.publish_pubsub_event(PubSubEvent::SubscriptionAdded {
            topic:      topic.clone(),
            subscriber: subscriber.label(),
          });
        }
      }
    }
  }
}

fn elapsed_millis(start: TimerInstant, end: TimerInstant) -> Option<u64> {
  if start.resolution() != end.resolution() {
    return None;
  }
  let resolution_ns = start.resolution().as_nanos().max(1);
  let ticks = end.ticks().saturating_sub(start.ticks());
  let elapsed_ns = ticks.saturating_mul(u64::try_from(resolution_ns).unwrap_or(u64::MAX));
  Some(elapsed_ns / 1_000_000)
}

fn mediator_owner_from_address(address: &str) -> UniqueAddress {
  let (host, port) = address_host_port(address);
  // UID 1 is an initialization placeholder; active membership rebinding replaces it before delivery
  // selection.
  UniqueAddress::new(Address::new("fraktor-cluster", host, port), 1)
}

fn mediator_owner_from_active_owners(
  advertised_address: &str,
  active_owners: &[UniqueAddress],
) -> Option<UniqueAddress> {
  let (host, port) = address_host_port(advertised_address);
  active_owners
    .iter()
    .find(|owner| normalize_host(owner.address().host()) == host && (port == 0 || owner.address().port() == port))
    .cloned()
}

fn owner_is_active_or_topology_member(
  owner: &UniqueAddress,
  active_owners: &[UniqueAddress],
  topology_members: &[String],
) -> bool {
  let topology_member = topology_members.iter().any(|member| owner_matches_member(owner, member));
  if active_owners.is_empty() {
    return topology_member;
  }
  topology_member && active_owners.iter().any(|active_owner| active_owner == owner)
}

fn owner_unavailable_in_topology(owner: &UniqueAddress, update: &TopologyUpdate) -> bool {
  owner_matches_any_member(owner, &update.left)
    || owner_matches_any_member(owner, &update.dead)
    || owner_matches_any_member(owner, &update.blocked)
}

fn owner_matches_any_member(owner: &UniqueAddress, members: &[String]) -> bool {
  members.iter().any(|member| owner_matches_member(owner, member))
}

fn owner_matches_member(owner: &UniqueAddress, member: &str) -> bool {
  let (host, port) = address_host_port(member);
  normalize_host(owner.address().host()) == host && (port == 0 || owner.address().port() == port)
}

fn address_host_port(address: &str) -> (String, u16) {
  if let Some(bracketed) = address.strip_prefix('[')
    && let Some((host, port_text)) = bracketed.split_once("]:")
    && let Ok(port) = port_text.parse::<u16>()
  {
    return (String::from(normalize_host(host)), port);
  }

  if let Some((host, port_text)) = address.rsplit_once(':')
    && !host.contains(':')
    && let Ok(port) = port_text.parse::<u16>()
  {
    (String::from(host), port)
  } else {
    (String::from(normalize_host(address)), 0)
  }
}

fn normalize_host(host: &str) -> &str {
  host.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(host)
}

impl ClusterPubSubImpl {
  /// Emits a metrics snapshot to the event stream.
  pub fn emit_metrics_snapshot(&mut self) {
    let _ = self.broker.drain_metrics();
    self.flush_broker_events_to_stream();
  }
}
