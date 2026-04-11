//! Actor-system scoped shared-factory package.

mod actor_shared_factory;
mod builtin_spin_shared_factory;
mod mailbox_shared_set;

pub use actor_shared_factory::ActorSharedFactory;
pub use builtin_spin_shared_factory::BuiltinSpinSharedFactory;
pub(crate) use mailbox_shared_set::MailboxLocked;
pub use mailbox_shared_set::MailboxSharedSet;
