//! Auto-detection for tick driver factories.

use fraktor_utils_core_rs::sync::ArcShared;

use super::{TickDriverError, TickDriverFactoryRef};
use crate::RuntimeToolbox;

/// Automatically detects and selects appropriate tick driver factory.
pub trait TickDriverAutoLocator<TB: RuntimeToolbox>: Send + Sync + 'static {
  /// Detects the appropriate driver factory for the current environment.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError::UnsupportedEnvironment`] if no suitable driver is found.
  fn detect(&self, toolbox: &TB) -> Result<TickDriverFactoryRef<TB>, TickDriverError>;

  /// Returns the default locator instance for this toolbox.
  fn default_ref() -> TickDriverAutoLocatorRef<TB>
  where
    Self: Sized;
}

/// Shared reference to a tick driver auto-locator.
pub type TickDriverAutoLocatorRef<TB> = ArcShared<dyn TickDriverAutoLocator<TB>>;
