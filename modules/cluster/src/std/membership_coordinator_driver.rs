//! std-only driver for membership coordination.

use alloc::string::String;

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamSharedGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess, time::TimerInstant};

use crate::core::{
  ClusterEvent, GossipTransport, MembershipCoordinatorError, MembershipCoordinatorOutcome,
  MembershipCoordinatorSharedGeneric,
};

/// Driver that applies coordinator outcomes to EventStream and gossip transport.
pub struct MembershipCoordinatorDriverGeneric<TB: RuntimeToolbox + 'static, TTransport: GossipTransport> {
  coordinator:  MembershipCoordinatorSharedGeneric<TB>,
  transport:    TTransport,
  event_stream: EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static, TTransport: GossipTransport> MembershipCoordinatorDriverGeneric<TB, TTransport> {
  /// Creates a new driver.
  #[must_use]
  pub fn new(
    coordinator: MembershipCoordinatorSharedGeneric<TB>,
    transport: TTransport,
    event_stream: EventStreamSharedGeneric<TB>,
  ) -> Self {
    Self { coordinator, transport, event_stream }
  }

  /// Returns the shared coordinator handle.
  #[must_use]
  pub const fn coordinator(&self) -> &MembershipCoordinatorSharedGeneric<TB> {
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
    now: TimerInstant,
  ) -> Result<(), MembershipCoordinatorError> {
    let outcome =
      self.coordinator.with_write(|coordinator| coordinator.handle_join(node_id.into(), authority.into(), now))?;
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
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }
}
