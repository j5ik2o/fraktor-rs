//! Execution scope for grain context.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::GrainContextGeneric;

#[cfg(test)]
mod tests;

/// Scope guard that controls the lifetime of a grain context.
pub struct GrainContextScope<TB: RuntimeToolbox + 'static> {
  context: Option<GrainContextGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> GrainContextScope<TB> {
  /// Creates a new scope from the provided context.
  #[must_use]
  pub const fn new(context: GrainContextGeneric<TB>) -> Self {
    Self { context: Some(context) }
  }

  /// Returns the current context if still active.
  #[must_use]
  pub const fn context(&self) -> Option<&GrainContextGeneric<TB>> {
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
