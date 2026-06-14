//! std-only driver for membership coordination.

use alloc::string::String;

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_cluster_core_kernel_rs::{
  extension::ClusterProviderShared,
  membership::{
    GossipTransport, MembershipCoordinatorError, MembershipCoordinatorOutcome, MembershipCoordinatorShared,
  },
  topology::ClusterEvent,
};
use fraktor_utils_core_rs::{sync::SharedAccess, time::TimerInstant};

use crate::{
  cluster_provider::StdSplitBrainResolverProvider,
  membership::split_brain_resolver_downing_driver::SplitBrainResolverDowningDriver,
};

#[cfg(test)]
#[path = "membership_coordinator_driver_test.rs"]
mod tests;

/// Driver that applies coordinator outcomes to EventStream and gossip transport.
pub(super) struct MembershipCoordinatorDriver<TTransport: GossipTransport> {
  coordinator:                 MembershipCoordinatorShared,
  transport:                   TTransport,
  event_stream:                EventStreamShared,
  split_brain_resolver_driver: Option<SplitBrainResolverDowningDriver>,
}

impl<TTransport: GossipTransport> MembershipCoordinatorDriver<TTransport> {
  /// Creates a new driver.
  #[must_use]
  pub(super) fn new(
    coordinator: MembershipCoordinatorShared,
    transport: TTransport,
    event_stream: EventStreamShared,
  ) -> Self {
    Self { coordinator, transport, event_stream, split_brain_resolver_driver: None }
  }

  /// Returns a driver that executes Split Brain Resolver downing targets during polling.
  #[must_use]
  pub(super) fn with_split_brain_resolver_downing(
    mut self,
    provider: StdSplitBrainResolverProvider,
    local_authority: impl Into<String>,
    cluster_provider: ClusterProviderShared,
  ) -> Self {
    self.split_brain_resolver_driver =
      Some(SplitBrainResolverDowningDriver::new(provider, local_authority.into(), cluster_provider));
    self
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
    self.apply_outcome(outcome)?;
    self.apply_split_brain_resolver_downing(now)?;
    Ok(())
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

  fn apply_split_brain_resolver_downing(&mut self, now: TimerInstant) -> Result<(), MembershipCoordinatorError> {
    let Some(driver) = self.split_brain_resolver_driver.as_mut() else {
      return Ok(());
    };
    let snapshot = self.coordinator.with_read(|coordinator| coordinator.snapshot());
    let authorities = driver.poll_downing_authorities(&snapshot, now);
    let mut provider_error = None;
    for authority in authorities {
      if let Some(driver) = self.split_brain_resolver_driver.as_ref() {
        match driver.down_cluster_provider(authority.as_str()) {
          | Ok(()) => {},
          | Err(_) if driver.is_local_authority(authority.as_str()) => {},
          | Err(error) => {
            if provider_error.is_none() {
              provider_error = Some(error);
            }
            continue;
          },
        }
      }
      let outcome = self.coordinator.with_write(|coordinator| coordinator.handle_down(authority.as_str(), now))?;
      self.apply_outcome(outcome)?;
    }
    if let Some(error) = provider_error {
      return Err(MembershipCoordinatorError::ClusterProvider(error));
    }
    Ok(())
  }
}
