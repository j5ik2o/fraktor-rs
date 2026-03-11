//! High-level typed actor bindings for the standard fraktor runtime.

/// Core typed actor primitives including actors, contexts, and references.
pub mod actor;
mod behavior;
mod behaviors;
mod props;
mod spawn_protocol;
mod stash_buffer;
mod system;
mod typed_ask_future;
mod typed_ask_response;

pub use behavior::*;
pub use behaviors::Behaviors;
pub use props::*;
pub use spawn_protocol::SpawnProtocol;
pub use stash_buffer::StashBuffer;
pub use system::*;
pub use typed_ask_future::*;
pub use typed_ask_response::*;
/// Alias for typed behavior lifecycle signals.
pub type BehaviorSignal = crate::core::typed::BehaviorSignal;
