//! Configuration registries for dispatchers and mailboxes.

/// Actor system configuration API.
mod actor_system_config;
/// Errors emitted by configuration registries.
mod config_error;
/// Dispatcher registry and associated utilities.
mod dispatchers;
/// Mailbox registry and associated utilities.
mod mailboxes;
/// Remoting configuration.
mod remoting_config;

pub use actor_system_config::ActorSystemConfig;
pub use config_error::ConfigError;
pub use dispatchers::{Dispatchers, DispatchersGeneric};
pub use mailboxes::{Mailboxes, MailboxesGeneric};
pub use remoting_config::RemotingConfig;
