//! System-level test helpers (gated by `test-support` feature).

#[cfg(feature = "test-support")]
mod empty_system;

#[cfg(feature = "test-support")]
pub use empty_system::{new_empty_actor_system, new_empty_actor_system_with, new_empty_typed_actor_system};
