//! Tracks the lifecycle of adapter references.

#[cfg(test)]
mod tests;

use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{actor_prim::Pid, system::SystemStateGeneric};

/// Lifecycle guard shared between adapter handles and senders.
pub struct AdapterLifecycleState<TB: RuntimeToolbox + 'static> {
  pid:    Pid,
  system: ArcShared<SystemStateGeneric<TB>>,
  alive:  AtomicBool,
}

impl<TB: RuntimeToolbox + 'static> AdapterLifecycleState<TB> {
  /// Creates a new lifecycle state bound to the target pid.
  #[must_use]
  pub const fn new(system: ArcShared<SystemStateGeneric<TB>>, pid: Pid) -> Self {
    Self { pid, system, alive: AtomicBool::new(true) }
  }

  /// Returns the target pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the shared system state.
  #[must_use]
  pub fn system(&self) -> ArcShared<SystemStateGeneric<TB>> {
    self.system.clone()
  }

  /// Returns whether the adapter is still alive.
  #[must_use]
  pub fn is_alive(&self) -> bool {
    self.alive.load(Ordering::SeqCst)
  }

  /// Marks the adapter as stopped.
  pub fn mark_stopped(&self) {
    self.alive.store(false, Ordering::SeqCst);
  }
}
