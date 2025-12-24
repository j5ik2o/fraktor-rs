use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Mailbox specialised for `StdToolbox`.
pub type Mailbox = crate::core::dispatch::mailbox::MailboxGeneric<StdToolbox>;
/// Mailbox offer future specialised for `StdToolbox`.
pub type MailboxOfferFuture = crate::core::dispatch::mailbox::MailboxOfferFutureGeneric<StdToolbox>;
/// Mailbox poll future specialised for `StdToolbox`.
pub type MailboxPollFuture = crate::core::dispatch::mailbox::MailboxPollFutureGeneric<StdToolbox>;
