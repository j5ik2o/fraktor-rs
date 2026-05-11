//! Marker helpers for classic actor logging.

#[cfg(test)]
#[path = "actor_log_marker_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String};

/// Marker metadata attached to classic actor log entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorLogMarker {
  name:       String,
  properties: BTreeMap<String, String>,
}

impl ActorLogMarker {
  /// Creates a marker with the provided name.
  #[must_use]
  pub fn new(name: impl Into<String>) -> Self {
    Self { name: name.into(), properties: BTreeMap::new() }
  }

  /// Creates the dead-letter marker described by Pekko's classic logging API.
  #[must_use]
  pub fn dead_letter(message_class: impl Into<String>) -> Self {
    Self::new("pekkoDeadLetter").with_property("pekkoMessageClass", message_class)
  }

  /// Adds a property to the marker.
  #[must_use]
  pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.properties.insert(key.into(), value.into());
    self
  }

  /// Returns the marker name.
  #[must_use]
  pub fn name(&self) -> &str {
    &self.name
  }

  /// Returns the marker properties.
  #[must_use]
  pub const fn properties(&self) -> &BTreeMap<String, String> {
    &self.properties
  }
}
