//! Confirmation message from the consumer to the consumer controller.

#[cfg(test)]
#[path = "consumer_controller_confirmed_test.rs"]
mod tests;

/// Confirmation that the consumer has processed the delivered message.
///
/// Sent from the consumer actor to the `ConsumerController` via the
/// `confirm_to` reference in [`ConsumerControllerDelivery`](super::ConsumerControllerDelivery).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsumerControllerConfirmed;
