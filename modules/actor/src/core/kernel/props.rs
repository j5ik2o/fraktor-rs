//! Props package.
//!
//! This module contains actor spawning configuration.

/// Props structure module.
mod base;
/// Actor factory module.
mod factory;
/// Shared wrapper for actor factory.
mod factory_shared;
/// Mailbox configuration module.
mod mailbox_config;
/// Mailbox configuration error module.
mod mailbox_config_error;
mod mailbox_requirement;
/// Supervisor options module.
mod supervisor_options;

pub use base::Props;
pub use factory::ActorFactory;
pub use factory_shared::ActorFactoryShared;
pub use mailbox_config::MailboxConfig;
pub use mailbox_config_error::MailboxConfigError;
pub use mailbox_requirement::MailboxRequirement;
pub use supervisor_options::SupervisorOptions;
