//! Setup package for actor system bootstrap configuration.
//!
//! Corresponds to `org.apache.pekko.actor.setup` in Pekko.

mod actor_system_config;
mod actor_system_setup;
mod bootstrap_setup;

pub use actor_system_config::ActorSystemConfig;
pub use actor_system_setup::ActorSystemSetup;
pub use bootstrap_setup::BootstrapSetup;
