//! Message that unsubscribes an actor from a `Listeners` host.

use crate::core::kernel::actor::actor_ref::ActorRef;

/// Unsubscribes an [`ActorRef`] from a [`Listeners`](super::Listeners) host.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Deafen`. Handled by
/// [`Listeners::handle`](super::Listeners::handle); unsubscribing a
/// [`Pid`](crate::core::kernel::actor::Pid) that is not currently registered
/// is a no-op and still counts as "handled".
#[derive(Clone, Debug)]
pub struct Deafen(pub ActorRef);
