//! Aggregates typed actor primitives.

mod actor;
mod actor_context;
mod actor_ref;
mod child_ref;

pub use actor::TypedActor;
pub use actor_context::{TypedActorContext, TypedActorContextGeneric};
pub use actor_ref::{TypedActorRef, TypedActorRefGeneric};
pub use child_ref::{TypedChildRef, TypedChildRefGeneric};
