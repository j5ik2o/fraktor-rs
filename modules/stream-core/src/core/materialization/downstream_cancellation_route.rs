use fraktor_actor_core_rs::core::kernel::actor::ChildRef;
#[cfg(any(test, feature = "test-support"))]
use fraktor_actor_core_rs::core::kernel::actor::Pid;

use crate::core::r#impl::{interpreter::IslandBoundaryShared, materialization::StreamShared};

pub(in crate::core::materialization) struct DownstreamCancellationRoute {
  boundary:             IslandBoundaryShared,
  upstream_stream:      StreamShared,
  downstream_stream:    StreamShared,
  upstream_actor:       ChildRef,
  cancel_command_count: u32,
}

impl DownstreamCancellationRoute {
  pub(in crate::core::materialization) const fn new(
    boundary: IslandBoundaryShared,
    upstream_stream: StreamShared,
    downstream_stream: StreamShared,
    upstream_actor: ChildRef,
  ) -> Self {
    Self { boundary, upstream_stream, downstream_stream, upstream_actor, cancel_command_count: 0 }
  }

  pub(in crate::core::materialization) fn should_propagate_cancellation(&self) -> bool {
    let downstream_terminal =
      self.downstream_stream.state().is_terminal() && !self.upstream_stream.state().is_terminal();
    (self.boundary.is_downstream_cancelled() || downstream_terminal) && self.cancel_command_count == 0
  }

  pub(in crate::core::materialization) const fn upstream_actor(&mut self) -> &mut ChildRef {
    &mut self.upstream_actor
  }

  pub(in crate::core::materialization) const fn record_cancel_command(&mut self) {
    self.cancel_command_count = self.cancel_command_count.saturating_add(1);
  }

  #[cfg(any(test, feature = "test-support"))]
  pub(in crate::core::materialization) fn cancel_command_count_for_actor(&self, actor_pid: Pid) -> u32 {
    if self.upstream_actor.pid() == actor_pid { self.cancel_command_count } else { 0 }
  }
}
