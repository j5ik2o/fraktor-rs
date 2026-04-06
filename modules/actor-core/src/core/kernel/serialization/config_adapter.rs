//! External configuration adapter integration.

use super::{builder::SerializationSetupBuilder, builder_error::SerializationBuilderError};

/// Applies external configuration sources to the builder.
pub trait SerializationConfigAdapter {
  /// Applies the adapter and returns the resulting builder.
  ///
  /// # Errors
  ///
  /// Returns an error if the configuration cannot be applied to the builder.
  fn apply(&self, builder: SerializationSetupBuilder) -> Result<SerializationSetupBuilder, SerializationBuilderError>;

  /// Provides metadata describing the adapter (used for diagnostics).
  fn metadata(&self) -> &'static str;
}
