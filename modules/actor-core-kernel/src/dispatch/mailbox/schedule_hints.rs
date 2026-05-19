/// Hint flags describing pending work types.
#[derive(Default, Clone, Copy, Debug)]
pub struct ScheduleHints {
  /// True when the system queue contains work.
  pub has_system_messages: bool,
  /// True when the user queue contains work.
  pub has_user_messages:   bool,
  /// True when the mailbox has signalled high backpressure.
  pub backpressure_active: bool,
}
