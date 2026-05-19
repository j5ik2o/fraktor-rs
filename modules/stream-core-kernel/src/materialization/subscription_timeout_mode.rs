/// Action taken when a subscription times out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionTimeoutMode {
  /// Do nothing on timeout.
  Noop,
  /// Log a warning on timeout.
  Warn,
  /// Cancel the stream on timeout.
  Cancel,
}
