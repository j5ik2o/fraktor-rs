//! Immutable router that dispatches messages via a pluggable routing logic.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use super::{
  broadcast::Broadcast, consistent_hashable_envelope::ConsistentHashableEnvelope, routee::Routee,
  routing_logic::RoutingLogic,
};
use crate::actor::{error::SendError, messaging::AnyMessage};

/// Routes messages to one or more routees using a configured [`RoutingLogic`].
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Router`.
///
/// The router follows an immutable-update pattern: mutation methods consume
/// `self` and return a new `Router` instance with the updated routee set.
/// The routing logic is shared (not cloned) across updates.
///
/// When no routees are registered, the router silently drops messages.
/// If observability is needed, add dead-letter publication, warning logs,
/// or hooks at routee management boundaries or around [`route`](Self::route).
pub struct Router<L: RoutingLogic> {
  logic:   L,
  routees: Vec<Routee>,
}

impl<L: RoutingLogic> Router<L> {
  /// Creates a new router with the given logic and initial routees.
  #[must_use]
  pub const fn new(logic: L, routees: Vec<Routee>) -> Self {
    Self { logic, routees }
  }

  /// Returns the current routees as a slice.
  #[must_use]
  pub fn routees(&self) -> &[Routee] {
    &self.routees
  }

  /// Routes a message through this router.
  ///
  /// If the message payload is a [`Broadcast`], the inner message is sent to
  /// all routees — even when that inner message is itself a
  /// [`ConsistentHashableEnvelope`], because Broadcast takes precedence over
  /// envelope unwrapping. Otherwise, the configured [`RoutingLogic`] selects
  /// a single routee for delivery. When the outer payload is a
  /// `ConsistentHashableEnvelope`, the selected routee receives the inner
  /// message rather than the envelope itself, matching Pekko's
  /// `RouterEnvelope` contract.
  ///
  /// If no routees are registered, this method returns `Ok(())` and drops
  /// the message. Integrate observability outside this method if that drop
  /// should surface as a dead-letter, warning, or custom hook.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the selected routee's underlying sender
  /// rejects the message.
  pub fn route(&mut self, message: AnyMessage) -> Result<(), SendError> {
    if let Some(broadcast) = message.downcast_ref::<Broadcast>() {
      // Broadcast wins over envelope unwrap: deliver the inner message as-is
      // to every routee, even if that inner message is a
      // ConsistentHashableEnvelope.
      let inner = broadcast.0.clone();
      let mut first_error = None;
      for routee in &mut self.routees {
        if let Err(error) = routee.send(inner.clone())
          && first_error.is_none()
        {
          first_error = Some(error);
        }
      }
      if let Some(error) = first_error {
        return Err(error);
      }
      return Ok(());
    }

    if self.routees.is_empty() {
      // No routees available — silently drop the message.
      return Ok(());
    }

    let idx = {
      let selected = self.logic.select(&message, &self.routees);
      if matches!(selected, Routee::NoRoutee) {
        // NoRoutee selected — silently drop (dead letter).
        return Ok(());
      }
      // Find the index of the selected routee via pointer equality.
      self.routees.iter().position(|r| core::ptr::eq(r, selected))
    };

    if let Some(i) = idx {
      // RouterEnvelope contract: if the message is a ConsistentHashableEnvelope,
      // strip it and deliver the inner message to the selected routee.
      let delivered = match message.downcast_ref::<ConsistentHashableEnvelope>() {
        | Some(envelope) => envelope.message().clone(),
        | None => message,
      };
      self.routees[i].send(delivered)?;
    }

    Ok(())
  }

  /// Returns a new router with the routee set replaced entirely.
  #[must_use]
  pub fn with_routees(self, routees: Vec<Routee>) -> Self {
    Self { logic: self.logic, routees }
  }

  /// Returns a new router with the given routee appended.
  #[must_use]
  pub fn add_routee(mut self, routee: Routee) -> Self {
    self.routees.push(routee);
    self
  }

  /// Returns a new router with the first matching routee removed.
  ///
  /// Matching is based on [`PartialEq`] for [`Routee`] (e.g. `ActorRef`
  /// compares by [`Pid`](crate::actor::Pid)).
  #[must_use]
  pub fn remove_routee(mut self, routee: &Routee) -> Self {
    if let Some(pos) = self.routees.iter().position(|r| r == routee) {
      self.routees.remove(pos);
    }
    self
  }
}
