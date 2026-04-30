use fraktor_actor_core_rs::core::kernel::actor::Pid;

use super::DownstreamCancellationRoute;

impl DownstreamCancellationRoute {
  pub(in crate::core::materialization) fn cancel_command_count_for_actor(&self, actor_pid: Pid) -> u32 {
    if self.upstream_actor.pid() == actor_pid { self.cancel_command_count } else { 0 }
  }
}
