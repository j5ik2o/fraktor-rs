//! Factory contract for [`MailboxSharedSet`](super::MailboxSharedSet).

use super::MailboxSharedSet;

/// Materializes [`MailboxSharedSet`] instances.
pub trait MailboxSharedSetFactory: Send + Sync {
  /// Creates a mailbox lock bundle.
  fn create(&self) -> MailboxSharedSet;
}
