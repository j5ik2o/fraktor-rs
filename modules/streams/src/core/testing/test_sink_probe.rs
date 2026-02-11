use alloc::vec::Vec;

use super::StreamError;

#[cfg(test)]
mod tests;

/// Test probe that acts as a demand-aware sink.
pub struct TestSinkProbe<T> {
  demand:    usize,
  received:  Vec<T>,
  completed: bool,
  failed:    Option<StreamError>,
}

impl<T> TestSinkProbe<T> {
  /// Creates an empty sink probe.
  #[must_use]
  pub const fn new() -> Self {
    Self { demand: 0, received: Vec::new(), completed: false, failed: None }
  }

  /// Requests downstream demand.
  pub const fn request(&mut self, amount: usize) {
    self.demand = self.demand.saturating_add(amount);
  }

  /// Attempts to push one element if demand is available.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::DemandExceeded`] when no demand is available.
  pub fn push(&mut self, value: T) -> Result<(), StreamError> {
    if self.demand == 0 {
      return Err(StreamError::DemandExceeded { requested: 1, remaining: 0 });
    }
    self.demand = self.demand.saturating_sub(1);
    self.received.push(value);
    Ok(())
  }

  /// Marks this probe as completed.
  pub const fn complete(&mut self) {
    self.completed = true;
  }

  /// Marks this probe as failed.
  pub const fn fail(&mut self, error: StreamError) {
    self.failed = Some(error);
  }

  /// Returns immutable view of received elements.
  #[must_use]
  pub fn received(&self) -> &[T] {
    &self.received
  }

  /// Returns true when completion was requested.
  #[must_use]
  pub const fn is_completed(&self) -> bool {
    self.completed
  }

  /// Returns failure if present.
  #[must_use]
  pub fn failed(&self) -> Option<StreamError> {
    self.failed.clone()
  }
}

impl<T> Default for TestSinkProbe<T> {
  fn default() -> Self {
    Self::new()
  }
}
