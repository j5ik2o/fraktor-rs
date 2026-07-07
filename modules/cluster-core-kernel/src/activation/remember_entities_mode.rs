//! Remembered entities store mode.

#[cfg(test)]
#[path = "remember_entities_mode_test.rs"]
mod tests;

/// Storage backend used for remembered entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RememberEntitiesStoreMode {
  /// Distributed data backed store.
  DData,
  /// Event-sourced store.
  EventSourced,
  /// Custom provider supplied by the runtime.
  Custom,
}

impl RememberEntitiesStoreMode {
  /// Returns the canonical configuration string for this mode.
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      | Self::DData => "ddata",
      | Self::EventSourced => "eventsourced",
      | Self::Custom => "custom",
    }
  }

  /// Parses a configuration string into a store mode.
  #[must_use]
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      | "ddata" => Some(Self::DData),
      | "eventsourced" => Some(Self::EventSourced),
      | "custom" => Some(Self::Custom),
      | _ => None,
    }
  }
}

impl Default for RememberEntitiesStoreMode {
  fn default() -> Self {
    Self::DData
  }
}
