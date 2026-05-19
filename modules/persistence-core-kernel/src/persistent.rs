//! Persistent actor package.

mod eventsourced;
mod pending_handler_invocation;
mod persistence_context;
mod persistent_actor;
mod persistent_actor_adapter;
mod persistent_actor_state;
mod persistent_envelope;
mod persistent_props;
mod persistent_repr;
mod recovery;
mod recovery_timed_out;
mod stash_overflow_strategy;

pub use eventsourced::Eventsourced;
pub use pending_handler_invocation::PendingHandlerInvocation;
pub use persistence_context::PersistenceContext;
pub use persistent_actor::PersistentActor;
pub(crate) use persistent_actor_adapter::PersistentActorAdapter;
pub use persistent_actor_state::PersistentActorState;
pub use persistent_envelope::PersistentEnvelope;
pub use persistent_props::{persistent_props, spawn_persistent};
pub use persistent_repr::PersistentRepr;
pub use recovery::Recovery;
pub use recovery_timed_out::RecoveryTimedOut;
pub use stash_overflow_strategy::StashOverflowStrategy;
