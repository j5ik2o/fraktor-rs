//! EventStream-based ClusterPubSub implementation backed by PubSubBroker.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeSet, format, string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::{
  event_stream::EventStreamGeneric,
  messaging::AnyMessageGeneric,
  serialization::{SerializationError, SerializationRegistryGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};

use crate::core::{
  ClusterEvent, ClusterPubSub, DeliverBatchRequest, DeliveryEndpointSharedGeneric, DeliveryReport, KindRegistry,
  PubSubBatch, PubSubBroker, PubSubConfig, PubSubError, PubSubEvent, PubSubSubscriber, PubSubTopic, PubSubTopicOptions,
  PublishAck, PublishOptions, PublishRejectReason, PublishRequest, StartupMode, SubscriberDeliveryReport,
  kind_registry::TOPIC_ACTOR_KIND,
};

/// PubSubBroker-backed ClusterPubSub implementation with EventStream integration.
///
/// This implementation requires TopicActorKind to be registered in the KindRegistry
/// before starting. On start, it creates the topic for TopicActorKind and publishes
/// events to EventStream.
pub struct ClusterPubSubImpl<TB: RuntimeToolbox + 'static> {
  event_stream:         ArcShared<EventStreamGeneric<TB>>,
  broker:               PubSubBroker<TB>,
  has_topic_actor_kind: bool,
  started:              bool,
  advertised_address:   String,
  pubsub_config:        PubSubConfig,
  delivery_endpoint:    DeliveryEndpointSharedGeneric<TB>,
  registry:             ArcShared<SerializationRegistryGeneric<TB>>,
  last_observed_at:     Option<TimerInstant>,
}

impl<TB: RuntimeToolbox + 'static> ClusterPubSubImpl<TB> {
  /// Creates a new PubSubImpl with EventStream and KindRegistry reference.
  ///
  /// The KindRegistry is checked for TopicActorKind presence at construction time.
  #[must_use]
  pub fn new(
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    registry: ArcShared<SerializationRegistryGeneric<TB>>,
    delivery_endpoint: DeliveryEndpointSharedGeneric<TB>,
    config: PubSubConfig,
    registry_snapshot: &KindRegistry,
  ) -> Self {
    let has_topic_actor_kind = registry_snapshot.contains(TOPIC_ACTOR_KIND);
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
    }
  }

  /// Creates a new PubSubImpl with a custom advertised address.
  #[must_use]
  pub fn with_advertised_address(mut self, address: impl Into<String>) -> Self {
    self.advertised_address = address.into();
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
    let payload = AnyMessageGeneric::new(event);
    let stream_event = fraktor_actor_rs::core::event_stream::EventStreamEvent::Extension {
      name: String::from("cluster-pubsub"),
      payload,
    };
    self.event_stream.publish(&stream_event);
  }

  fn publish_cluster_event(&self, event: ClusterEvent) {
    let payload = AnyMessageGeneric::new(event);
    let stream_event =
      fraktor_actor_rs::core::event_stream::EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&stream_event);
  }

  fn effective_options(&mut self, topic: &PubSubTopic, overrides: PublishOptions) -> Option<PubSubTopicOptions> {
    let defaults = self.broker.topic_options(topic).ok()?;
    Some(defaults.apply_overrides(&overrides))
  }

  fn serialize_payload(&self, payload: &AnyMessageGeneric<TB>) -> Result<PubSubBatch, SerializationError> {
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
    let envelope =
      crate::core::PubSubEnvelope { serializer_id: serializer.identifier().value(), type_name: name, bytes };
    Ok(PubSubBatch::new(vec![envelope]))
  }

  fn map_serialization_error(error: &SerializationError) -> Result<PublishAck, PubSubError> {
    if error.is_not_serializable() {
      return Ok(PublishAck::rejected(PublishRejectReason::NotSerializable));
    }
    Err(PubSubError::SerializationFailed { reason: format!("{error:?}") })
  }

  fn split_subscribers(
    subscribers: Vec<PubSubSubscriber<TB>>,
  ) -> (Vec<PubSubSubscriber<TB>>, Vec<PubSubSubscriber<TB>>) {
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

  fn deliver_group(
    &mut self,
    topic: &PubSubTopic,
    batch: PubSubBatch,
    subscribers: &[PubSubSubscriber<TB>],
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

  fn handle_delivery_report(
    &mut self,
    topic: &PubSubTopic,
    subscribers: &[PubSubSubscriber<TB>],
    report: DeliveryReport<TB>,
  ) {
    let now = self.last_observed_at.unwrap_or_else(|| TimerInstant::from_ticks(0, Duration::from_secs(1)));

    let mut failed_set = BTreeSet::new();
    for SubscriberDeliveryReport { subscriber, status } in report.failed {
      failed_set.insert(subscriber.clone());
      let _ = self.broker.suspend_subscriber(topic, &subscriber, format!("{status:?}"), now);
      self.publish_pubsub_event(PubSubEvent::DeliveryFailed {
        topic: topic.clone(),
        subscriber: subscriber.label(),
        status,
      });
    }

    for subscriber in subscribers {
      if !failed_set.contains(subscriber) {
        self.publish_pubsub_event(PubSubEvent::DeliverySucceeded {
          topic:      topic.clone(),
          subscriber: subscriber.label(),
        });
      }
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterPubSub<TB> for ClusterPubSubImpl<TB> {
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

  fn subscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    let result = self.broker.subscribe(topic, &subscriber);
    self.flush_broker_events_to_stream();
    result
  }

  fn unsubscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    if !self.started {
      return Err(PubSubError::NotStarted);
    }
    let result = self.broker.unsubscribe(topic, &subscriber);
    self.flush_broker_events_to_stream();
    result
  }

  fn publish(&mut self, request: PublishRequest<TB>) -> Result<PublishAck, PubSubError> {
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

  fn on_topology(&mut self, _topology: &crate::core::ClusterTopology) {
    let now =
      self.last_observed_at.unwrap_or_else(|| TimerInstant::zero(Duration::from_secs(1))).saturating_add_ticks(1);
    self.last_observed_at = Some(now);
    for topic in self.broker.topics() {
      if let Ok(removed) = self.broker.remove_expired_suspended(&topic, now, self.pubsub_config.suspended_ttl) {
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

impl<TB: RuntimeToolbox + 'static> ClusterPubSubImpl<TB> {
  /// Emits a metrics snapshot to the event stream.
  pub fn emit_metrics_snapshot(&mut self) {
    let _ = self.broker.drain_metrics();
    self.flush_broker_events_to_stream();
  }
}
