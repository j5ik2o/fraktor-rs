use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::actor::{ChildRef, Pid};

use crate::r#impl::{interpreter::IslandBoundaryShared, materialization::StreamShared};

#[cfg(test)]
#[path = "downstream_cancellation_route_test.rs"]
mod tests;

struct DownstreamCancellationWatch {
  boundary:          IslandBoundaryShared,
  downstream_stream: StreamShared,
}

pub(crate) struct DownstreamCancellationRoute {
  downstream_watches:   Vec<DownstreamCancellationWatch>,
  upstream_stream:      StreamShared,
  upstream_actor:       ChildRef,
  cancel_in_flight:     bool,
  cancel_command_count: u32,
}

pub(crate) struct ReservedDownstreamCancellationTarget {
  actor_pid: Pid,
  actor:     ChildRef,
}

impl ReservedDownstreamCancellationTarget {
  pub(crate) const fn actor_pid(&self) -> Pid {
    self.actor_pid
  }

  pub(crate) fn into_actor(self) -> ChildRef {
    self.actor
  }
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
      cancel_in_flight: false,
      cancel_command_count: 0,
    }
  }

  pub(crate) fn add_downstream(&mut self, boundary: IslandBoundaryShared, downstream_stream: StreamShared) {
    self.downstream_watches.push(DownstreamCancellationWatch { boundary, downstream_stream });
  }

  pub(crate) fn should_propagate_cancellation(&self) -> bool {
    if self.cancel_command_count != 0 || self.cancel_in_flight || self.upstream_stream.state().is_terminal() {
      return false;
    }

    self.downstream_watches.iter().all(|watch| {
      let downstream_terminal = watch.downstream_stream.state().is_terminal();
      watch.boundary.is_downstream_cancelled() || downstream_terminal
    })
  }

  pub(crate) fn reserve_cancel_target(&mut self) -> Option<ReservedDownstreamCancellationTarget> {
    if !self.should_propagate_cancellation() {
      return None;
    }
    self.cancel_in_flight = true;
    Some(ReservedDownstreamCancellationTarget {
      actor_pid: self.upstream_actor.pid(),
      actor:     self.upstream_actor.clone(),
    })
  }

  pub(crate) fn finish_cancel_delivery(&mut self, actor_pid: Pid, delivered: bool) -> bool {
    if self.upstream_actor.pid() != actor_pid {
      return false;
    }
    self.cancel_in_flight = false;
    if delivered {
      self.cancel_command_count = self.cancel_command_count.saturating_add(1);
    }
    true
  }
}
