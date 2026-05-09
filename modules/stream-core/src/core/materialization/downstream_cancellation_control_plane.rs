use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_actor_core_kernel_rs::actor::Pid;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::downstream_cancellation_route::{DownstreamCancellationRoute, ReservedDownstreamCancellationTarget};

#[cfg(test)]
mod tests;

pub(crate) struct DownstreamCancellationControlPlane {
  routes: Vec<DownstreamCancellationRoute>,
}

/// Shared handle to a [`DownstreamCancellationControlPlane`].
///
/// Carries an additional `pending` flag so island actors can fast-skip
/// the inner mutex when no boundary has signalled a downstream cancel
/// since the last propagation cycle.
#[derive(Clone)]
pub(crate) struct DownstreamCancellationControlPlaneShared {
  inner:   ArcShared<SpinSyncMutex<DownstreamCancellationControlPlane>>,
  pending: Arc<AtomicBool>,
}

impl DownstreamCancellationControlPlaneShared {
  pub(crate) fn new(plane: DownstreamCancellationControlPlane) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(plane)), pending: Arc::new(AtomicBool::new(false)) }
  }

  /// Runs `f` while holding the inner mutex. Closure-based to avoid
  /// re-entry footguns (see `.agents/rules/rust/immutability-policy.md`).
  pub(crate) fn with_locked<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut DownstreamCancellationControlPlane) -> R, {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }

  /// Returns a clone of the pending-cancellation signal so boundaries can
  /// arm it from outside the mutex.
  pub(crate) fn pending_signal(&self) -> Arc<AtomicBool> {
    self.pending.clone()
  }

  /// Returns `true` and clears the flag if a propagation cycle is needed.
  pub(crate) fn take_pending(&self) -> bool {
    self.pending.swap(false, Ordering::AcqRel)
  }

  /// Re-arms the flag. Used by the propagator when it processed targets,
  /// so any failed deliveries are retried on the next drive without
  /// requiring a fresh boundary signal.
  pub(crate) fn arm_pending(&self) {
    self.pending.store(true, Ordering::Release);
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
