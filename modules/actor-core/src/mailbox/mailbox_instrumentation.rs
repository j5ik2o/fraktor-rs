//! Mailbox metrics instrumentation placeholder.

/// Provides mailbox metrics publication facilities.
#[derive(Clone, Default)]
pub struct MailboxInstrumentation;

impl MailboxInstrumentation {
  /// Creates a new instrumentation helper.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Publishes a metrics snapshot.
  pub fn publish(&self, _user_len: usize, _system_len: usize) {}
}
