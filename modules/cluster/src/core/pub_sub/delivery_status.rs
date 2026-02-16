//! Delivery status classification.

/// Outcome status for delivery attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
  /// Delivery succeeded.
  Delivered,
  /// Delivery timed out.
  Timeout,
  /// Subscriber could not be reached.
  SubscriberUnreachable,
  /// Other delivery error.
  OtherError,
}
