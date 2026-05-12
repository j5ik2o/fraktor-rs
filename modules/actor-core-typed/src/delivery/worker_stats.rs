//! Statistics about registered workers in the work-pulling producer controller.

#[cfg(test)]
#[path = "worker_stats_test.rs"]
mod tests;

/// Statistics about registered workers.
///
/// Returned in response to a
/// [`GetWorkerStats`](super::WorkPullingProducerControllerCommand) query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerStats {
  number_of_workers: usize,
}

impl WorkerStats {
  /// Creates a new `WorkerStats`.
  pub(crate) const fn new(number_of_workers: usize) -> Self {
    Self { number_of_workers }
  }

  /// Returns the number of currently registered workers.
  #[must_use]
  pub const fn number_of_workers(&self) -> usize {
    self.number_of_workers
  }
}
