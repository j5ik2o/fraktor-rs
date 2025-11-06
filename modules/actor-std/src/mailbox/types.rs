use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Mailbox specialised for `StdToolbox`.
pub type Mailbox = cellactor_actor_core_rs::mailbox::MailboxGeneric<StdToolbox>;
/// Mailbox offer future specialised for `StdToolbox`.
pub type MailboxOfferFuture = cellactor_actor_core_rs::mailbox::MailboxOfferFutureGeneric<StdToolbox>;
/// Mailbox poll future specialised for `StdToolbox`.
pub type MailboxPollFuture = cellactor_actor_core_rs::mailbox::MailboxPollFutureGeneric<StdToolbox>;
