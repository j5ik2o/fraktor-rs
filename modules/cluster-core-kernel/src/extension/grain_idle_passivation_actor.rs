//! Actor that runs Grain idle-passivation maintenance outside the scheduler lock.

use fraktor_actor_core_kernel_rs::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView, scheduler::SchedulerShared},
  event::stream::EventStreamShared,
};
use fraktor_utils_core_rs::sync::SharedLock;

use super::{cluster_extension::publish_activation_events, scheduler_time::scheduler_time_secs};
use crate::{ClusterCore, grain::GrainMetricsShared};

pub(super) struct GrainIdlePassivationActor {
  core:          SharedLock<ClusterCore>,
  event_stream:  EventStreamShared,
  grain_metrics: Option<GrainMetricsShared>,
  scheduler:     SchedulerShared,
}

impl GrainIdlePassivationActor {
  pub(super) const fn new(
    core: SharedLock<ClusterCore>,
    event_stream: EventStreamShared,
    grain_metrics: Option<GrainMetricsShared>,
    scheduler: SchedulerShared,
  ) -> Self {
    Self { core, event_stream, grain_metrics, scheduler }
  }
}

impl Actor for GrainIdlePassivationActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<()>().is_none() {
      return Ok(());
    }
    let now = scheduler_time_secs(&self.scheduler);
    let events = self.core.with_lock(|core| {
      core.passivate_idle(now);
      core.drain_placement_events()
    });
    publish_activation_events(&self.event_stream, &self.grain_metrics, events);
    Ok(())
  }
}
