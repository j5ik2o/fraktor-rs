//! System-level test helpers (gated by `test-support` feature at parent module).

mod actor_system_config;
mod empty_system;

pub use actor_system_config::std_actor_system_config;
pub use empty_system::{new_noop_actor_system, new_noop_actor_system_with};
