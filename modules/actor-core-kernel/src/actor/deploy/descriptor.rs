use alloc::string::String;

use super::Scope;

#[cfg(test)]
#[path = "descriptor_test.rs"]
mod tests;

/// Immutable deployment description for classic actor configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deploy {
  path:  Option<String>,
  scope: Scope,
}

impl Deploy {
  /// Creates a new local deployment.
  #[must_use]
  pub const fn new() -> Self {
    Self { path: None, scope: Scope::Local }
  }

  /// Attaches a logical deployment path.
  #[must_use]
  pub fn with_path(mut self, path: impl Into<String>) -> Self {
    self.path = Some(path.into());
    self
  }

  /// Replaces the deployment scope.
  #[must_use]
  pub fn with_scope(mut self, scope: Scope) -> Self {
    self.scope = scope;
    self
  }

  /// Returns the configured deployment path.
  #[must_use]
  pub fn path(&self) -> Option<&str> {
    self.path.as_deref()
  }

  /// Returns the configured scope.
  #[must_use]
  pub const fn scope(&self) -> &Scope {
    &self.scope
  }
}

impl Default for Deploy {
  fn default() -> Self {
    Self::new()
  }
}
