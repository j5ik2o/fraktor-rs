//! Props package.
//!
//! This module contains actor spawning configuration.

/// Dispatcher configuration module.
mod dispatcher_config;
/// Actor factory module.
mod factory;
/// Mailbox configuration module.
mod mailbox_config;
/// Props structure module.
mod props;
/// Supervisor options module.
mod supervisor_options;

pub use dispatcher_config::DispatcherConfig;
pub use factory::ActorFactory;
pub use mailbox_config::MailboxConfig;
pub use props::Props;
pub use supervisor_options::SupervisorOptions;
