use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::ChildRef;

use crate::core::r#impl::{interpreter::IslandBoundaryShared, materialization::StreamShared};

#[cfg(test)]
mod tests;

struct DownstreamCancellationWatch {
  boundary:          IslandBoundaryShared,
  downstream_stream: StreamShared,
}

pub(crate) struct DownstreamCancellationRoute {
  downstream_watches:   Vec<DownstreamCancellationWatch>,
  upstream_stream:      StreamShared,
  upstream_actor:       ChildRef,
  cancel_command_count: u32,
}

impl DownstreamCancellationRoute {
  pub(crate) fn new(
    boundary: IslandBoundaryShared,
    upstream_stream: StreamShared,
    downstream_stream: StreamShared,
    upstream_actor: ChildRef,
  ) -> Self {
    Self {
      downstream_watches: alloc::vec![DownstreamCancellationWatch { boundary, downstream_stream }],
      upstream_stream,
      upstream_actor,
      cancel_command_count: 0,
    }
  }

  pub(crate) fn add_downstream(&mut self, boundary: IslandBoundaryShared, downstream_stream: StreamShared) {
    self.downstream_watches.push(DownstreamCancellationWatch { boundary, downstream_stream });
  }

  pub(crate) fn should_propagate_cancellation(&self) -> bool {
    if self.cancel_command_count != 0 || self.upstream_stream.state().is_terminal() {
      return false;
    }

    self.downstream_watches.iter().all(|watch| {
      let downstream_terminal = watch.downstream_stream.state().is_terminal();
      watch.boundary.is_downstream_cancelled() || downstream_terminal
    })
  }

  pub(crate) const fn upstream_actor(&mut self) -> &mut ChildRef {
    &mut self.upstream_actor
  }

  pub(crate) const fn record_cancel_command(&mut self) {
    self.cancel_command_count = self.cancel_command_count.saturating_add(1);
  }
}
