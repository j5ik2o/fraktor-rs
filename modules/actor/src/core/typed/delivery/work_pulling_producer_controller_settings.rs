//! Configuration for the work-pulling producer controller.

#[cfg(test)]
mod tests;

/// Default buffer size for buffered messages awaiting worker demand.
const DEFAULT_BUFFER_SIZE: u32 = 1000;

/// Settings for
/// [`WorkPullingProducerController`](super::WorkPullingProducerController).
#[derive(Debug, Clone)]
pub(crate) struct WorkPullingProducerControllerSettings {
  buffer_size: u32,
}

impl WorkPullingProducerControllerSettings {
  /// Creates default settings.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { buffer_size: DEFAULT_BUFFER_SIZE }
  }

  /// Returns the maximum number of messages buffered while waiting for worker
  /// demand.
  #[must_use]
  pub(crate) const fn buffer_size(&self) -> u32 {
    self.buffer_size
  }
}

impl Default for WorkPullingProducerControllerSettings {
  fn default() -> Self {
    Self::new()
  }
}
