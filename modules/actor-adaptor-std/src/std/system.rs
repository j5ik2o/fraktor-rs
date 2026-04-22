//! System-level test helpers (gated by `test-support` feature at parent module).

mod empty_system;

pub use empty_system::{new_empty_actor_system, new_empty_actor_system_with};
