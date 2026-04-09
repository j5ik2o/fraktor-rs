//! Actor-system scoped lock-provider package.

mod actor_lock_provider;
mod builtin_spin_lock_provider;
mod debug_spin_lock;
mod debug_spin_lock_provider;
mod mailbox_shared_set;
mod shared_lock;

pub use actor_lock_provider::ActorLockProvider;
pub use builtin_spin_lock_provider::BuiltinSpinLockProvider;
pub(crate) use debug_spin_lock::{DebugSpinLock, DebugSpinLockGuard};
pub use debug_spin_lock_provider::DebugSpinLockProvider;
pub(crate) use mailbox_shared_set::MailboxLocked;
pub use mailbox_shared_set::MailboxSharedSet;
pub(crate) use shared_lock::SharedLock;
