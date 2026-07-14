//! Actor that runs Grain idle-passivation maintenance outside the scheduler lock.

use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView},
  event::stream::EventStreamShared,
};
use fraktor_utils_core_rs::sync::{SharedAccess, SharedLock};

use super::cluster_extension::publish_activation_events;
use crate::{ClusterCore, grain::GrainMetricsShared};

pub(super) struct GrainIdlePassivationActor {
  core:          SharedLock<ClusterCore>,
  event_stream:  EventStreamShared,
  grain_metrics: Option<GrainMetricsShared>,
}

impl GrainIdlePassivationActor {
  pub(super) const fn new(
    core: SharedLock<ClusterCore>,
    event_stream: EventStreamShared,
    grain_metrics: Option<GrainMetricsShared>,
  ) -> Self {
    Self { core, event_stream, grain_metrics }
  }
}

impl Actor for GrainIdlePassivationActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    let Some(now) = message.downcast_ref::<u64>().copied() else {
      return Ok(());
    };
    let events = self.core.with_write(|core| {
      if core.mode().is_none() {
        return Vec::new();
      }
      core.passivate_idle(now);
      core.drain_placement_events()
    });
    publish_activation_events(&self.event_stream, &self.grain_metrics, events);
    Ok(())
  }
}
