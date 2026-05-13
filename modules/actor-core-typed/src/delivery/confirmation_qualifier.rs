//! Confirmation qualifier for durable producer queue.

#[cfg(test)]
#[path = "confirmation_qualifier_test.rs"]
mod tests;

use alloc::string::String;

/// Type alias for confirmation qualifiers used to distinguish independent
/// delivery chains within a single `ProducerController`.
///
/// Corresponds to Pekko's `DurableProducerQueue.ConfirmationQualifier`.
pub type ConfirmationQualifier = String;

/// The default qualifier used when no specific qualifier is needed.
///
/// Corresponds to Pekko's `DurableProducerQueue.NoQualifier`.
pub const NO_QUALIFIER: ConfirmationQualifier = String::new();
