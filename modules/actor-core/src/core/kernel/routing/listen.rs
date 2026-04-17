//! Message that subscribes an actor to a `Listeners` host.

use crate::core::kernel::actor::actor_ref::ActorRef;

/// Subscribes an [`ActorRef`] to a [`Listeners`](super::Listeners) host.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Listen`. Handled by
/// [`Listeners::handle`](super::Listeners::handle) and is idempotent with
/// respect to the listener's [`Pid`](crate::core::kernel::actor::Pid):
/// re-subscribing the same `Pid` does not grow the listener set.
#[derive(Clone, Debug)]
pub struct Listen(pub ActorRef);
