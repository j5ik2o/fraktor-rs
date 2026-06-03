//! Delivery mode selected by the mediator.

/// Mediator delivery mode for selected delivery targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediatorDeliveryMode {
  /// All matching topic subscribers selected for `Publish`.
  Publish,
  /// One target selected for `Send`.
  Send,
  /// All matching targets selected for `SendToAll`.
  SendToAll,
}
