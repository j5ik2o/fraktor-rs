//! Actor construction descriptors.

mod actor_factory;
mod dispatcher_config;
mod mailbox_config;
mod props_struct;
mod supervisor_options;

pub use actor_factory::ActorFactory;
#[allow(unused_imports)]
pub use dispatcher_config::DispatcherConfig;
#[allow(unused_imports)]
pub use mailbox_config::MailboxConfig;
pub use props_struct::Props;
#[allow(unused_imports)]
pub use supervisor_options::SupervisorOptions;
