//! High-level typed actor bindings for the standard Cellactor runtime.

/// Core typed actor primitives including actors, contexts, and references.
pub mod actor_prim;
mod behavior;
mod behaviors;
mod props;
mod system;
mod typed_ask_future;
mod typed_ask_response;

pub use behavior::*;
pub use behaviors::Behaviors;
pub use props::*;
pub use system::*;
pub use typed_ask_future::*;
pub use typed_ask_response::*;

/// Type alias for behavior signals bound to the standard runtime toolbox.
pub type BehaviorSignal = cellactor_actor_core_rs::typed::BehaviorSignal;
