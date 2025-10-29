//! Actor future placeholder implementation.

/// Represents an asynchronous result produced by an actor interaction.
#[derive(Debug)]
pub struct ActorFuture<T> {
  value: Option<T>,
  on_complete: Option<fn(&T)>,
}

impl<T> ActorFuture<T> {
  /// Creates a pending future with no completion value.
  #[must_use]
  pub const fn pending() -> Self {
    Self { value: None, on_complete: None }
  }

  /// Registers a completion callback invoked when the future resolves.
  pub fn set_on_complete(&mut self, callback: fn(&T)) {
    self.on_complete = Some(callback);
  }

  /// Completes the future with the provided value.
  pub fn complete(&mut self, value: T) {
    self.value = Some(value);
    if let (Some(callback), Some(ref stored)) = (self.on_complete, self.value.as_ref()) {
      callback(stored);
    }
  }

  /// Returns `true` when the future has been completed.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.value.is_some()
  }

  /// Extracts the completion value.
  pub fn take(&mut self) -> Option<T> {
    self.value.take()
  }
}

impl<T> Default for ActorFuture<T> {
  fn default() -> Self {
    Self::pending()
  }
}
