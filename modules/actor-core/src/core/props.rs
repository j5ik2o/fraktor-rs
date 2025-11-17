//! Props package.
//!
//! This module contains actor spawning configuration.

/// Props structure module.
mod base;
/// Dispatcher configuration module.
mod dispatcher_config;
/// Actor factory module.
mod factory;
/// Mailbox configuration module.
mod mailbox_config;
mod mailbox_requirement;
/// Supervisor options module.
mod supervisor_options;

pub use base::{Props, PropsGeneric};
pub use dispatcher_config::{DispatcherConfig, DispatcherConfigGeneric};
pub use factory::ActorFactory;
pub use mailbox_config::MailboxConfig;
pub use mailbox_requirement::MailboxRequirement;
pub use supervisor_options::SupervisorOptions;
