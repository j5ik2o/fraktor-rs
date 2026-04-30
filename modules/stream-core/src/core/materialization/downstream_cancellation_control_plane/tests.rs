use fraktor_actor_core_rs::core::kernel::actor::Pid;

use super::DownstreamCancellationControlPlane;
use crate::core::StreamError;

impl DownstreamCancellationControlPlane {
  pub(in crate::core::materialization) fn cancel_command_count_for_actor(&self, actor_pid: Pid) -> u32 {
    self.routes.iter().map(|route| route.cancel_command_count_for_actor(actor_pid)).sum()
  }
}

#[test]
fn replace_routes_resets_recorded_cancellation_failure() {
  let mut control_plane = DownstreamCancellationControlPlane::new(Vec::new());
  control_plane.first_failure = Some(StreamError::Failed);

  control_plane.replace_routes(Vec::new());
  let result = control_plane.propagate(|_| Ok(()));

  assert_eq!(result, Ok(()));
}
