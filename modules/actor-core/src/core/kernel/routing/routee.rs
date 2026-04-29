//! Routee abstraction for message delivery.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::kernel::actor::{actor_ref::ActorRef, error::SendError, messaging::AnyMessage};

/// Represents a destination for routed messages.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Routee` family:
/// `ActorRefRoutee`, `NoRoutee`, and `SeveralRoutees`.
#[derive(Clone, Debug)]
pub enum Routee {
  /// Routes a message to a single actor via its [`ActorRef`].
  ActorRef(ActorRef),
  /// A no-op routee that silently drops messages.
  NoRoutee,
  /// Broadcasts a message to all contained routees.
  Several(Vec<Routee>),
}

impl Routee {
  /// Sends a message to this routee.
  ///
  /// - `ActorRef`: delegates to [`ActorRef::try_tell`].
  /// - `NoRoutee`: returns `Ok(())` (message is silently dropped).
  /// - `Several`: sends to each contained routee in order; remembers the first error while still
  ///   attempting delivery to the remaining routees.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] when the underlying sender rejects the message.
  pub fn send(&mut self, message: AnyMessage) -> Result<(), SendError> {
    match self {
      | Self::ActorRef(actor_ref) => actor_ref.try_tell(message),
      | Self::NoRoutee => Ok(()),
      | Self::Several(routees) => {
        let mut first_error = None;
        for routee in routees.iter_mut() {
          if let Err(error) = routee.send(message.clone())
            && first_error.is_none()
          {
            first_error = Some(error);
          }
        }
        match first_error {
          | Some(error) => Err(error),
          | None => Ok(()),
        }
      },
    }
  }
}

impl PartialEq for Routee {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      | (Self::ActorRef(a), Self::ActorRef(b)) => a == b,
      | (Self::NoRoutee, Self::NoRoutee) => true,
      | (Self::Several(a), Self::Several(b)) => a == b,
      | _ => false,
    }
  }
}

impl Eq for Routee {}
