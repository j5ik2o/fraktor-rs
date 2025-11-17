use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

/// Mailbox specialised for `StdToolbox`.
pub type Mailbox = crate::core::mailbox::MailboxGeneric<StdToolbox>;
/// Mailbox offer future specialised for `StdToolbox`.
pub type MailboxOfferFuture = crate::core::mailbox::MailboxOfferFutureGeneric<StdToolbox>;
/// Mailbox poll future specialised for `StdToolbox`.
pub type MailboxPollFuture = crate::core::mailbox::MailboxPollFutureGeneric<StdToolbox>;
