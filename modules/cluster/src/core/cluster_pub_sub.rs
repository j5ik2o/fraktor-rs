//! Abstraction over cluster-wide pub/sub control.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishRequest, TopologyUpdate};

/// Starts and stops the cluster pub/sub subsystem.
pub trait ClusterPubSub<TB: RuntimeToolbox>: Send + Sync {
  /// Starts pub/sub services.
  ///
  /// # Errors
  ///
  /// Returns an error if the pub/sub subsystem fails to start.
  fn start(&mut self) -> Result<(), PubSubError>;

  /// Stops pub/sub services.
  ///
  /// # Errors
  ///
  /// Returns an error if the pub/sub subsystem fails to stop.
  fn stop(&mut self) -> Result<(), PubSubError>;

  /// Subscribes to a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the subscription fails.
  fn subscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError>;

  /// Unsubscribes from a topic.
  ///
  /// # Errors
  ///
  /// Returns an error if the unsubscription fails.
  fn unsubscribe(&mut self, topic: &PubSubTopic, subscriber: PubSubSubscriber<TB>) -> Result<(), PubSubError>;

  /// Publishes to a topic and returns acknowledgement.
  ///
  /// # Errors
  ///
  /// Returns an error only for system-level failures.
  fn publish(&mut self, request: PublishRequest<TB>) -> Result<PublishAck, PubSubError>;

  /// Applies a topology update to refresh routing decisions.
  fn on_topology(&mut self, update: &TopologyUpdate);
}
