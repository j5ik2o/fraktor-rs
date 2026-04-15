//! Setup package for actor system bootstrap configuration.
//!
//! Corresponds to `org.apache.pekko.actor.setup` in Pekko.

mod actor_system_config;
mod actor_system_setup;
mod bootstrap_setup;
mod circuit_breaker_settings;

pub use actor_system_config::ActorSystemConfig;
pub use actor_system_setup::ActorSystemSetup;
pub use bootstrap_setup::BootstrapSetup;
pub use circuit_breaker_settings::CircuitBreakerSettings;
