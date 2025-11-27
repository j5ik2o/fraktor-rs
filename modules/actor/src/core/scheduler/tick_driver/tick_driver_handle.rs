//! Handle owning the lifetime of a running tick driver instance.

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{TickDriverControl, TickDriverId, TickDriverKind};

/// Handle owning the lifetime of a running tick driver instance.
pub struct TickDriverHandleGeneric<TB: RuntimeToolbox> {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
  control:    ArcShared<ToolboxMutex<Box<dyn TickDriverControl>, TB>>,
}

impl<TB: RuntimeToolbox> Clone for TickDriverHandleGeneric<TB> {
  fn clone(&self) -> Self {
    Self { id: self.id, kind: self.kind, resolution: self.resolution, control: self.control.clone() }
  }
}

impl<TB: RuntimeToolbox> TickDriverHandleGeneric<TB> {
  /// Creates a new driver handle.
  #[must_use]
  pub fn new(
    id: TickDriverId,
    kind: TickDriverKind,
    resolution: Duration,
    control: ArcShared<ToolboxMutex<Box<dyn TickDriverControl>, TB>>,
  ) -> Self {
    Self { id, kind, resolution, control }
  }

  /// Returns the associated driver identifier.
  #[must_use]
  pub const fn id(&self) -> TickDriverId {
    self.id
  }

  /// Returns the driver classification kind.
  #[must_use]
  pub const fn kind(&self) -> TickDriverKind {
    self.kind
  }

  /// Returns the tick resolution produced by the driver.
  #[must_use]
  pub const fn resolution(&self) -> Duration {
    self.resolution
  }

  /// Stops the underlying driver.
  pub fn shutdown(&self) {
    self.control.lock().shutdown();
  }
}

/// Type alias for `TickDriverHandleGeneric` with the default `NoStdToolbox`.
pub type TickDriverHandle = TickDriverHandleGeneric<NoStdToolbox>;
