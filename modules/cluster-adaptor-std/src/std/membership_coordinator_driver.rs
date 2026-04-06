//! std-only driver for membership coordination.

use alloc::string::String;

use fraktor_actor_core_rs::core::kernel::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_cluster_core_rs::core::{
  ClusterEvent, ClusterExtensionConfig,
  membership::{
    GossipTransport, MembershipCoordinatorError, MembershipCoordinatorOutcome, MembershipCoordinatorShared,
  },
};
use fraktor_utils_rs::core::{sync::SharedAccess, time::TimerInstant};

/// Driver that applies coordinator outcomes to EventStream and gossip transport.
pub struct MembershipCoordinatorDriver<TTransport: GossipTransport> {
  coordinator:  MembershipCoordinatorShared,
  transport:    TTransport,
  event_stream: EventStreamShared,
}

impl<TTransport: GossipTransport> MembershipCoordinatorDriver<TTransport> {
  /// Creates a new driver.
  #[must_use]
  pub fn new(coordinator: MembershipCoordinatorShared, transport: TTransport, event_stream: EventStreamShared) -> Self {
    Self { coordinator, transport, event_stream }
  }

  /// Returns the shared coordinator handle.
  #[must_use]
  pub const fn coordinator(&self) -> &MembershipCoordinatorShared {
    &self.coordinator
  }

  /// Returns a mutable reference to the gossip transport.
  pub fn transport_mut(&mut self) -> &mut TTransport {
    &mut self.transport
  }

  /// Handles a join request through the coordinator.
  pub fn handle_join(
    &mut self,
    node_id: impl Into<String>,
    authority: impl Into<String>,
    joining_config: &ClusterExtensionConfig,
    now: TimerInstant,
  ) -> Result<(), MembershipCoordinatorError> {
    let outcome = self
      .coordinator
      .with_write(|coordinator| coordinator.handle_join(node_id.into(), authority.into(), joining_config, now))?;
    self.apply_outcome(outcome)
  }

  /// Handles a leave request through the coordinator.
  pub fn handle_leave(&mut self, authority: &str, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let outcome = self.coordinator.with_write(|coordinator| coordinator.handle_leave(authority, now))?;
    self.apply_outcome(outcome)
  }

  /// Handles a heartbeat through the coordinator.
  pub fn handle_heartbeat(&mut self, authority: &str, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let outcome = self.coordinator.with_write(|coordinator| coordinator.handle_heartbeat(authority, now))?;
    self.apply_outcome(outcome)
  }

  /// Polls incoming gossip deltas and applies them.
  pub fn handle_gossip_deltas(&mut self, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let deltas = self.transport.poll_deltas();
    for (peer, delta) in deltas {
      let outcome = self.coordinator.with_write(|coordinator| coordinator.handle_gossip_delta(&peer, &delta, now))?;
      self.apply_outcome(outcome)?;
    }
    Ok(())
  }

  /// Handles a quarantine event from transport.
  pub fn handle_quarantine(
    &mut self,
    authority: &str,
    reason: &str,
    now: TimerInstant,
  ) -> Result<(), MembershipCoordinatorError> {
    let outcome = self
      .coordinator
      .with_write(|coordinator| coordinator.handle_quarantine(authority.to_string(), reason.to_string(), now))?;
    self.apply_outcome(outcome)
  }

  /// Polls coordinator timers to emit topology updates.
  pub fn poll(&mut self, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
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
