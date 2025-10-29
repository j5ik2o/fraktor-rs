use crate::mailbox_policy::{MailboxPolicy, OverflowPolicy};

/// Mailbox-specific configuration stored within [`Props`].
#[derive(Clone)]
pub struct MailboxConfig {
  policy:             MailboxPolicy,
  throughput_limit:   u32,
  warning_threshold:  Option<usize>,
  system_queue_ratio: f32,
}

impl Default for MailboxConfig {
  fn default() -> Self {
    Self {
      policy:             MailboxPolicy::bounded(64, OverflowPolicy::DropNewest),
      throughput_limit:   300,
      warning_threshold:  None,
      system_queue_ratio: 0.25,
    }
  }
}

impl MailboxConfig {
  /// Returns the configured mailbox policy.
  #[must_use]
  pub const fn policy(&self) -> &MailboxPolicy {
    &self.policy
  }

  /// Updates the mailbox policy.
  #[must_use]
  pub fn with_policy(mut self, policy: MailboxPolicy) -> Self {
    self.policy = policy;
    self
  }

  /// Returns the throughput limit per scheduling turn.
  #[must_use]
  pub const fn throughput_limit(&self) -> u32 {
    self.throughput_limit
  }

  /// Updates the throughput limit.
  #[must_use]
  pub fn with_throughput_limit(mut self, limit: u32) -> Self {
    self.throughput_limit = limit;
    self
  }

  /// Returns the optional occupancy threshold that triggers warnings.
  #[must_use]
  pub const fn warning_threshold(&self) -> Option<usize> {
    self.warning_threshold
  }

  /// Configures the occupancy threshold for warning events.
  #[must_use]
  pub fn with_warning_threshold(mut self, threshold: Option<usize>) -> Self {
    self.warning_threshold = threshold;
    self
  }

  /// Ratio of reserved capacity for system messages.
  #[must_use]
  pub const fn system_queue_ratio(&self) -> f32 {
    self.system_queue_ratio
  }

  /// Configures the system queue capacity ratio.
  #[must_use]
  pub fn with_system_queue_ratio(mut self, ratio: f32) -> Self {
    self.system_queue_ratio = ratio.clamp(0.0, 1.0);
    self
  }
}
