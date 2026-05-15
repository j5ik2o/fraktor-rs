//! Props package.
//!
//! This module contains actor spawning configuration.

/// Props structure module.
mod base;
mod deployable_actor_factory;
mod deployable_actor_factory_registry;
mod deployable_factory_error;
mod deployable_factory_lookup_error;
mod deployable_props_metadata;
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
pub use deployable_actor_factory::DeployableActorFactory;
pub use deployable_actor_factory_registry::DeployableActorFactoryRegistry;
pub use deployable_factory_error::DeployableFactoryError;
pub use deployable_factory_lookup_error::DeployableFactoryLookupError;
pub use deployable_props_metadata::DeployablePropsMetadata;
pub use factory::ActorFactory;
pub use factory_shared::ActorFactoryShared;
pub use mailbox_config::MailboxConfig;
pub use mailbox_config_error::MailboxConfigError;
pub use mailbox_requirement::MailboxRequirement;
pub use supervisor_options::SupervisorOptions;
