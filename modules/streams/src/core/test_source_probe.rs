use alloc::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Test probe that acts as a controllable source.
pub struct TestSourceProbe<T> {
  queued:    VecDeque<T>,
  completed: bool,
}

impl<T> TestSourceProbe<T> {
  /// Creates an empty source probe.
  #[must_use]
  pub const fn new() -> Self {
    Self { queued: VecDeque::new(), completed: false }
  }

  /// Enqueues an element for downstream pull.
  pub fn push(&mut self, value: T) {
    self.queued.push_back(value);
  }

  /// Marks this probe as completed.
  pub const fn complete(&mut self) {
    self.completed = true;
  }

  /// Pulls the next queued element.
  #[must_use]
  pub fn pull(&mut self) -> Option<T> {
    self.queued.pop_front()
  }

  /// Returns true when completion was requested.
  #[must_use]
  pub const fn is_completed(&self) -> bool {
    self.completed
  }
}

impl<T> Default for TestSourceProbe<T> {
  fn default() -> Self {
    Self::new()
  }
}
