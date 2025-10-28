#[cfg(test)]
mod tests;

/// Enumeration representing the size limit of a queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSize {
  /// No limit (unlimited).
  Limitless,
  /// Limited to the specified size.
  Limited(usize),
}

impl QueueSize {
  /// Constant constructor representing an unlimited size queue.
  #[must_use]
  pub const fn limitless() -> Self {
    Self::Limitless
  }

  /// Constant constructor representing a queue limited to the specified size.
  #[must_use]
  pub const fn limited(value: usize) -> Self {
    Self::Limited(value)
  }

  /// Determines whether this size is unlimited.
  #[must_use]
  pub const fn is_limitless(&self) -> bool {
    matches!(self, Self::Limitless)
  }

  /// Gets the size as `usize`. Returns `usize::MAX` if unlimited.
  #[must_use]
  pub const fn to_usize(self) -> usize {
    match self {
      | Self::Limitless => usize::MAX,
      | Self::Limited(value) => value,
    }
  }
}

impl Default for QueueSize {
  fn default() -> Self {
    QueueSize::limited(0)
  }
}
