//! Commands accepted by the distributed pub-sub mediator protocol.

#[cfg(test)]
#[path = "mediator_command_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_actor_core_kernel_rs::serialization::SerializerId;

use super::{MediatorPathKey, MediatorQuery, PubSubEnvelope, PubSubError, PubSubSubscriber, PubSubTopic};

/// Command contract accepted by the distributed pub-sub mediator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorCommand {
  /// Registers a local actor target under a path key.
  Put {
    /// Canonical address-less path key.
    path:   MediatorPathKey,
    /// Registered target.
    target: PubSubSubscriber,
  },
  /// Removes a path registration.
  Remove {
    /// Canonical address-less path key.
    path: MediatorPathKey,
  },
  /// Subscribes a target to a topic.
  Subscribe {
    /// Topic name.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
  /// Unsubscribes a target from a topic.
  Unsubscribe {
    /// Topic name.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
  /// Publishes a serialized payload to topic subscribers.
  Publish {
    /// Topic name.
    topic:   PubSubTopic,
    /// Serialized mediator payload.
    payload: PubSubEnvelope,
  },
  /// Sends a serialized payload to one matching path target.
  Send {
    /// Canonical address-less path key.
    path:           MediatorPathKey,
    /// Serialized mediator payload.
    payload:        PubSubEnvelope,
    /// Whether local owner entries should be preferred.
    local_affinity: bool,
  },
  /// Sends a serialized payload to all matching path targets.
  SendToAll {
    /// Canonical address-less path key.
    path:         MediatorPathKey,
    /// Serialized mediator payload.
    payload:      PubSubEnvelope,
    /// Whether the local owner entry should be skipped.
    all_but_self: bool,
  },
  /// Reads mediator registry state.
  Query(MediatorQuery),
}

impl MediatorCommand {
  /// Creates a validated `Put` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] when `path` is not a canonical actor path.
  pub fn try_put(path: &str, target: PubSubSubscriber) -> Result<Self, PubSubError> {
    Ok(Self::Put { path: MediatorPathKey::parse(path)?, target })
  }

  /// Creates a validated `Remove` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] when `path` is not a canonical actor path.
  pub fn try_remove(path: &str) -> Result<Self, PubSubError> {
    Ok(Self::Remove { path: MediatorPathKey::parse(path)? })
  }

  /// Creates a validated `Subscribe` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidTopic`] when `topic` is empty.
  pub fn try_subscribe(
    topic: PubSubTopic,
    group: Option<String>,
    subscriber: PubSubSubscriber,
  ) -> Result<Self, PubSubError> {
    Self::validate_topic(&topic)?;
    Ok(Self::Subscribe { topic, group, subscriber })
  }

  /// Creates a validated `Unsubscribe` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidTopic`] when `topic` is empty.
  pub fn try_unsubscribe(
    topic: PubSubTopic,
    group: Option<String>,
    subscriber: PubSubSubscriber,
  ) -> Result<Self, PubSubError> {
    Self::validate_topic(&topic)?;
    Ok(Self::Unsubscribe { topic, group, subscriber })
  }

  /// Creates a validated `Publish` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidTopic`] or [`PubSubError::InvalidPayload`] when validation
  /// fails.
  pub fn try_publish(topic: PubSubTopic, payload: PubSubEnvelope) -> Result<Self, PubSubError> {
    Self::validate_topic(&topic)?;
    Self::validate_payload(&payload)?;
    Ok(Self::Publish { topic, payload })
  }

  /// Creates a validated `Send` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] or [`PubSubError::InvalidPayload`] when validation fails.
  pub fn try_send(path: &str, payload: PubSubEnvelope, local_affinity: bool) -> Result<Self, PubSubError> {
    Self::validate_payload(&payload)?;
    Ok(Self::Send { path: MediatorPathKey::parse(path)?, payload, local_affinity })
  }

  /// Creates a validated `SendToAll` command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidPath`] or [`PubSubError::InvalidPayload`] when validation fails.
  pub fn try_send_to_all(path: &str, payload: PubSubEnvelope, all_but_self: bool) -> Result<Self, PubSubError> {
    Self::validate_payload(&payload)?;
    Ok(Self::SendToAll { path: MediatorPathKey::parse(path)?, payload, all_but_self })
  }

  /// Creates a current-topics query command.
  #[must_use]
  pub const fn current_topics() -> Self {
    Self::Query(MediatorQuery::CurrentTopics)
  }

  /// Creates a validated subscriber-count query command.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::InvalidTopic`] when `topic` is empty.
  pub fn subscriber_count(topic: PubSubTopic) -> Result<Self, PubSubError> {
    Self::validate_topic(&topic)?;
    Ok(Self::Query(MediatorQuery::SubscriberCount { topic }))
  }

  fn validate_topic(topic: &PubSubTopic) -> Result<(), PubSubError> {
    if topic.is_empty() {
      return Err(PubSubError::InvalidTopic { reason: String::from("topic must not be empty") });
    }
    Ok(())
  }

  fn validate_payload(payload: &PubSubEnvelope) -> Result<(), PubSubError> {
    SerializerId::try_from(payload.serializer_id)
      .map_err(|_| PubSubError::InvalidPayload { reason: String::from("payload serializer_id is reserved") })?;
    if payload.type_name.is_empty() {
      return Err(PubSubError::InvalidPayload { reason: String::from("payload type_name must not be empty") });
    }
    if payload.bytes.is_empty() {
      return Err(PubSubError::InvalidPayload { reason: String::from("payload bytes must not be empty") });
    }
    Ok(())
  }
}
