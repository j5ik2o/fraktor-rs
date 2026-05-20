use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::actor::Pid;
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::downstream_cancellation_route::{DownstreamCancellationRoute, ReservedDownstreamCancellationTarget};

#[cfg(test)]
#[path = "downstream_cancellation_control_plane_test.rs"]
mod tests;

pub(crate) struct DownstreamCancellationControlPlane {
  routes: Vec<DownstreamCancellationRoute>,
}

/// Shared handle to a [`DownstreamCancellationControlPlane`].
#[derive(Clone)]
pub(crate) struct DownstreamCancellationControlPlaneShared {
  inner: ArcShared<SpinSyncMutex<DownstreamCancellationControlPlane>>,
}

impl DownstreamCancellationControlPlaneShared {
  pub(crate) fn new(plane: DownstreamCancellationControlPlane) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(plane)) }
  }

  /// Runs `f` while holding the inner mutex. Closure-based to avoid
  /// re-entry footguns (see `.agents/rules/rust/immutability-policy.md`).
  pub(crate) fn with_locked<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut DownstreamCancellationControlPlane) -> R, {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

pub(crate) fn empty_shared() -> DownstreamCancellationControlPlaneShared {
  DownstreamCancellationControlPlaneShared::new(DownstreamCancellationControlPlane::new(Vec::new()))
}

impl DownstreamCancellationControlPlane {
  pub(crate) const fn new(routes: Vec<DownstreamCancellationRoute>) -> Self {
    Self { routes }
  }

  pub(crate) fn replace_routes(&mut self, routes: Vec<DownstreamCancellationRoute>) {
    self.routes = routes;
  }

  pub(crate) fn reserve_cancellation_targets(&mut self) -> Vec<ReservedDownstreamCancellationTarget> {
    let mut targets = Vec::new();
    for route in &mut self.routes {
      if let Some(target) = route.reserve_cancel_target() {
        targets.push(target);
      }
    }
    targets
  }

  pub(crate) fn finish_cancellation_delivery(&mut self, actor_pid: Pid, delivered: bool) {
    for route in &mut self.routes {
      if route.finish_cancel_delivery(actor_pid, delivered) {
        break;
      }
    }
  }
}
