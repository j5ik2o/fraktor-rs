//! Actor selection path element carried by remote selection messages.

#[cfg(test)]
#[path = "selection_path_element_test.rs"]
mod tests;

use alloc::string::String;

/// One step in an actor selection path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionPathElement {
  /// Selects a child by exact actor name.
  ChildName(String),
  /// Selects children matching a wildcard pattern.
  ChildPattern(String),
  /// Selects the parent actor.
  Parent,
}
