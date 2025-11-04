use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

/// Mailbox specialised for `StdToolbox`.
pub type Mailbox = cellactor_actor_core_rs::mailbox::Mailbox<StdToolbox>;
/// Mailbox offer future specialised for `StdToolbox`.
pub type MailboxOfferFuture = cellactor_actor_core_rs::mailbox::MailboxOfferFuture<StdToolbox>;
/// Mailbox poll future specialised for `StdToolbox`.
pub type MailboxPollFuture = cellactor_actor_core_rs::mailbox::MailboxPollFuture<StdToolbox>;
