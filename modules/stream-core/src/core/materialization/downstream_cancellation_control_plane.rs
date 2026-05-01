use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::Pid;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::downstream_cancellation_route::{DownstreamCancellationRoute, ReservedDownstreamCancellationTarget};

#[cfg(test)]
mod tests;

pub(crate) struct DownstreamCancellationControlPlane {
  routes: Vec<DownstreamCancellationRoute>,
}

pub(crate) type DownstreamCancellationControlPlaneShared = ArcShared<SpinSyncMutex<DownstreamCancellationControlPlane>>;

pub(crate) fn empty_shared() -> DownstreamCancellationControlPlaneShared {
  ArcShared::new(SpinSyncMutex::new(DownstreamCancellationControlPlane::new(Vec::new())))
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
