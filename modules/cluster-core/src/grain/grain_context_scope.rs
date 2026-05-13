//! Execution scope for grain context.

use super::GrainContextImpl;

#[cfg(test)]
#[path = "grain_context_scope_test.rs"]
mod tests;

/// Scope guard that controls the lifetime of a grain context.
pub struct GrainContextScope {
  context: Option<GrainContextImpl>,
}

impl GrainContextScope {
  /// Creates a new scope from the provided context.
  #[must_use]
  pub const fn new(context: GrainContextImpl) -> Self {
    Self { context: Some(context) }
  }

  /// Returns the current context if still active.
  #[must_use]
  pub const fn context(&self) -> Option<&GrainContextImpl> {
    self.context.as_ref()
  }

  /// Returns whether the context is still active.
  #[must_use]
  pub const fn is_active(&self) -> bool {
    self.context.is_some()
  }

  /// Finishes the scope and releases the context.
  pub fn finish(&mut self) {
    self.context = None;
  }
}
