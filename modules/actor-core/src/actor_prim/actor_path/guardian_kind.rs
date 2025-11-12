//! Guardian hierarchy definitions.

/// Guardian hierarchy that anchors the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardianKind {
  /// `/system` guardian target.
  System,
  /// `/user` guardian target.
  User,
}

impl GuardianKind {
  #[must_use]
  /// Returns the textual guardian segment.
  pub const fn segment(&self) -> &'static str {
    match self {
      | GuardianKind::System => "system",
      | GuardianKind::User => "user",
    }
  }
}
