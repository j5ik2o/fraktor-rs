use fraktor_utils_core_rs::std::runtime_toolbox::StdToolbox;

/// Mailbox specialised for `StdToolbox`.
pub type Mailbox = fraktor_actor_core_rs::core::mailbox::MailboxGeneric<StdToolbox>;
/// Mailbox offer future specialised for `StdToolbox`.
pub type MailboxOfferFuture = fraktor_actor_core_rs::core::mailbox::MailboxOfferFutureGeneric<StdToolbox>;
/// Mailbox poll future specialised for `StdToolbox`.
pub type MailboxPollFuture = fraktor_actor_core_rs::core::mailbox::MailboxPollFutureGeneric<StdToolbox>;
