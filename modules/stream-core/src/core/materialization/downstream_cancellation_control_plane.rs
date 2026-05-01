use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::ChildRef;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::downstream_cancellation_route::DownstreamCancellationRoute;
use crate::core::StreamError;

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

  pub(crate) fn propagate<F>(&mut self, mut cancel_actor: F) -> Result<(), StreamError>
  where
    F: FnMut(&mut ChildRef) -> Result<(), StreamError>, {
    let mut result = Ok(());
    for route in &mut self.routes {
      if !route.should_propagate_cancellation() {
        continue;
      }
      match cancel_actor(route.upstream_actor()) {
        | Ok(()) => {
          route.record_cancel_command();
        },
        | Err(error) => {
          if result.is_ok() {
            result = Err(error);
          }
        },
      }
    }
    result
  }
}
