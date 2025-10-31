//! Mailbox capacity and overflow policies.

mod mailbox_capacity;
mod mailbox_overflow_strategy;
mod mailbox_policy_struct;

pub use mailbox_capacity::MailboxCapacity;
pub use mailbox_overflow_strategy::MailboxOverflowStrategy;
pub use mailbox_policy_struct::MailboxPolicy;
