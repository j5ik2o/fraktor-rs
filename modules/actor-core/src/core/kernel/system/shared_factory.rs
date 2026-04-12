//! Actor-system scoped shared-factory package.

mod mailbox_shared_set;

pub(crate) use mailbox_shared_set::MailboxLocked;
pub use mailbox_shared_set::MailboxSharedSet;
