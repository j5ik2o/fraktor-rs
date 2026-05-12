//! Envelope that carries an explicit consistent-hash key alongside its payload.

#[cfg(test)]
#[path = "consistent_hashable_envelope_test.rs"]
mod tests;

use super::consistent_hashable::ConsistentHashable;
use crate::actor::messaging::AnyMessage;

/// Wraps a message with an explicit consistent-hash key.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.ConsistentHashableEnvelope`.
///
/// When a [`Router`](super::Router) receives a message whose payload is a
/// `ConsistentHashableEnvelope`, the configured
/// [`ConsistentHashingRoutingLogic`](super::ConsistentHashingRoutingLogic)
/// uses the envelope's [`hash_key`](Self::hash_key) directly and skips the
/// registered hash-key mapper. After a routee is selected, the router unwraps
/// the envelope and delivers the inner
/// [`message`](Self::message) instead of the envelope itself, matching
/// Pekko's `RouterEnvelope` contract.
#[derive(Debug)]
pub struct ConsistentHashableEnvelope {
  message:  AnyMessage,
  hash_key: u64,
}

impl ConsistentHashableEnvelope {
  /// Creates a new envelope that carries `message` and the given `hash_key`.
  #[must_use]
  pub const fn new(message: AnyMessage, hash_key: u64) -> Self {
    Self { message, hash_key }
  }

  /// Returns the explicit consistent-hash key.
  #[must_use]
  pub const fn hash_key(&self) -> u64 {
    self.hash_key
  }

  /// Returns a reference to the inner message.
  #[must_use]
  pub const fn message(&self) -> &AnyMessage {
    &self.message
  }
}

impl Clone for ConsistentHashableEnvelope {
  fn clone(&self) -> Self {
    Self { message: self.message.clone(), hash_key: self.hash_key }
  }
}

impl ConsistentHashable for ConsistentHashableEnvelope {
  fn consistent_hash_key(&self) -> u64 {
    self.hash_key
  }
}
