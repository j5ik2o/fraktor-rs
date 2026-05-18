//! Durable state load result.

#[cfg(test)]
#[path = "get_object_result_test.rs"]
mod tests;

/// Result returned when loading a durable state object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetObjectResult<A> {
  value:    Option<A>,
  revision: u64,
}

impl<A> GetObjectResult<A> {
  /// Creates a durable state load result.
  #[must_use]
  pub const fn new(value: Option<A>, revision: u64) -> Self {
    Self { value, revision }
  }

  /// Creates an empty durable state load result.
  #[must_use]
  pub const fn empty() -> Self {
    Self { value: None, revision: 0 }
  }

  /// Returns the loaded value, when present.
  #[must_use]
  pub const fn value(&self) -> Option<&A> {
    self.value.as_ref()
  }

  /// Consumes the result and returns the loaded value.
  #[must_use]
  pub fn into_value(self) -> Option<A> {
    self.value
  }

  /// Returns the durable state revision.
  #[must_use]
  pub const fn revision(&self) -> u64 {
    self.revision
  }

  /// Returns true when no value was found.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.value.is_none()
  }
}
