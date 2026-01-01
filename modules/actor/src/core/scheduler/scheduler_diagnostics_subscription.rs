use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{
  SchedulerDiagnosticsEvent,
  diagnostics_registry::{DiagnosticsBufferGeneric, DiagnosticsRegistryGeneric},
};

/// Handle returned to diagnostics subscribers for draining events.
pub struct SchedulerDiagnosticsSubscriptionGeneric<TB: RuntimeToolbox + 'static> {
  id:       u64,
  registry: DiagnosticsRegistryGeneric<TB>,
  buffer:   ArcShared<DiagnosticsBufferGeneric<TB>>,
  detached: bool,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub type SchedulerDiagnosticsSubscription = SchedulerDiagnosticsSubscriptionGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SchedulerDiagnosticsSubscriptionGeneric<TB> {
  pub(crate) const fn new(
    id: u64,
    registry: DiagnosticsRegistryGeneric<TB>,
    buffer: ArcShared<DiagnosticsBufferGeneric<TB>>,
  ) -> Self {
    Self { id, registry, buffer, detached: false }
  }

  /// Drains and returns all pending diagnostics events.
  #[must_use]
  pub fn drain(&mut self) -> Vec<SchedulerDiagnosticsEvent> {
    self.buffer.drain()
  }
}

impl<TB: RuntimeToolbox + 'static> Drop for SchedulerDiagnosticsSubscriptionGeneric<TB> {
  fn drop(&mut self) {
    if !self.detached {
      self.registry.remove(self.id);
      self.detached = true;
    }
  }
}
