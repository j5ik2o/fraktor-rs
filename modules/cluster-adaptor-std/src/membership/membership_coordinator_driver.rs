//! std-only driver for membership coordination.

use alloc::string::String;

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_cluster_core_kernel_rs::{
  membership::{
    GossipTransport, MembershipCoordinatorError, MembershipCoordinatorOutcome, MembershipCoordinatorShared,
  },
  topology::ClusterEvent,
};
use fraktor_utils_core_rs::{sync::SharedAccess, time::TimerInstant};

#[cfg(test)]
#[path = "membership_coordinator_driver_test.rs"]
mod tests;

/// Driver that applies coordinator outcomes to EventStream and gossip transport.
pub(super) struct MembershipCoordinatorDriver<TTransport: GossipTransport> {
  coordinator:  MembershipCoordinatorShared,
  transport:    TTransport,
  event_stream: EventStreamShared,
}

impl<TTransport: GossipTransport> MembershipCoordinatorDriver<TTransport> {
  /// Creates a new driver.
  #[must_use]
  pub(super) fn new(
    coordinator: MembershipCoordinatorShared,
    transport: TTransport,
    event_stream: EventStreamShared,
  ) -> Self {
    Self { coordinator, transport, event_stream }
  }

  /// Polls incoming gossip deltas and applies them.
  pub(super) fn handle_gossip_deltas(&mut self, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let deltas = self.transport.poll_deltas();
    for (peer, delta) in deltas {
      let outcome = self.coordinator.with_write(|coordinator| coordinator.handle_gossip_delta(&peer, &delta, now))?;
      self.apply_outcome(outcome)?;
    }
    Ok(())
  }

  /// Polls coordinator timers to emit topology updates.
  pub(super) fn poll(&mut self, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let outcome = self.coordinator.with_write(|coordinator| coordinator.poll(now))?;
    self.apply_outcome(outcome)
  }

  fn apply_outcome(&mut self, outcome: MembershipCoordinatorOutcome) -> Result<(), MembershipCoordinatorError> {
    if let Some(event) = outcome.topology_event {
      self.publish_event(event);
    }
    for event in outcome.member_events {
      self.publish_event(event);
    }
    for outbound in outcome.gossip_outbound {
      self.transport.send(outbound).map_err(MembershipCoordinatorError::Transport)?;
    }
    Ok(())
  }

  fn publish_event(&self, event: ClusterEvent) {
    let payload = AnyMessage::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }
}
