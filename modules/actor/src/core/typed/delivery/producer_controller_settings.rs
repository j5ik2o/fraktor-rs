//! Configuration for the producer controller.

#[cfg(test)]
mod tests;

/// Settings for [`ProducerController`](super::ProducerController).
///
/// This covers only the in-memory reliable delivery settings.
/// Durable queue settings are out of scope for the current implementation.
#[derive(Debug, Clone)]
pub(crate) struct ProducerControllerSettings;

impl ProducerControllerSettings {
  /// Creates default settings for in-memory reliable delivery.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }
}

impl Default for ProducerControllerSettings {
  fn default() -> Self {
    Self::new()
  }
}
