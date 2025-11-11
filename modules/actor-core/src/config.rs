//! Configuration registries for dispatchers and mailboxes.

/// Errors emitted by configuration registries.
mod config_error;
/// Dispatcher registry and associated utilities.
mod dispatchers;
/// Mailbox registry and associated utilities.
mod mailboxes;

pub use config_error::ConfigError;
pub use dispatchers::{Dispatchers, DispatchersGeneric};
pub use mailboxes::{Mailboxes, MailboxesGeneric};
