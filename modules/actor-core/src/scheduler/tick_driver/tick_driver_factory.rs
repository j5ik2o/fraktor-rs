//! Factory trait for creating tick drivers.

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{TickDriver, TickDriverError, TickDriverKind};
use crate::RuntimeToolbox;

/// Factory for creating tick driver instances.
pub trait TickDriverFactory<TB: RuntimeToolbox>: Send + Sync + 'static {
  /// Returns the kind of driver this factory produces.
  fn kind(&self) -> TickDriverKind;

  /// Returns the tick resolution of drivers created by this factory.
  fn resolution(&self) -> Duration;

  /// Builds a new tick driver instance.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] if driver creation fails.
  fn build(&self) -> Result<Box<dyn TickDriver<TB>>, TickDriverError>;
}

/// Shared reference to a tick driver factory.
pub type TickDriverFactoryRef<TB> = ArcShared<dyn TickDriverFactory<TB>>;
