use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::ChildRef;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::downstream_cancellation_route::DownstreamCancellationRoute;
use crate::core::StreamError;

#[cfg(test)]
mod tests;

pub(in crate::core::materialization) struct DownstreamCancellationControlPlane {
  routes:        Vec<DownstreamCancellationRoute>,
  first_failure: Option<StreamError>,
}

pub(in crate::core::materialization) type DownstreamCancellationControlPlaneShared =
  ArcShared<SpinSyncMutex<DownstreamCancellationControlPlane>>;

impl DownstreamCancellationControlPlane {
  pub(in crate::core::materialization) const fn new(routes: Vec<DownstreamCancellationRoute>) -> Self {
    Self { routes, first_failure: None }
  }

  pub(in crate::core::materialization) fn replace_routes(&mut self, routes: Vec<DownstreamCancellationRoute>) {
    self.routes = routes;
    self.first_failure = None;
  }

  #[must_use]
  pub(in crate::core::materialization) const fn route_count(&self) -> usize {
    self.routes.len()
  }

  pub(in crate::core::materialization) fn propagate<F>(&mut self, mut cancel_actor: F) -> Result<(), StreamError>
  where
    F: FnMut(&mut ChildRef) -> Result<(), StreamError>, {
    if let Some(error) = &self.first_failure {
      return Err(error.clone());
    }
    for route in &mut self.routes {
      if !route.should_propagate_cancellation() {
        continue;
      }
      match cancel_actor(route.upstream_actor()) {
        | Ok(()) => {
          route.record_cancel_command();
        },
        | Err(error) => {
          self.first_failure = Some(error.clone());
          return Err(error);
        },
      }
    }
    Ok(())
  }
}
