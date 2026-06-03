//! Delivery mode selected by the mediator.

/// Mediator delivery mode for selected delivery targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediatorDeliveryMode {
  /// One target selected for `Send`.
  Send,
  /// All matching targets selected for `SendToAll`.
  SendToAll,
}
