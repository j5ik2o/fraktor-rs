//! Scheduler and lifecycle state owned by SystemState.

#[cfg(test)]
#[path = "scheduler_lifecycle_registry_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::sync::ArcShared;
use portable_atomic::AtomicBool;

use super::super::termination_state::TerminationState;
use crate::{
  actor::scheduler::{
    SchedulerContext,
    tick_driver::{TickDriverBundle, TickDriverStopper},
  },
  event::stream::TickDriverSnapshot,
};

/// Owns scheduler handles and actor system lifecycle state.
pub(crate) struct SchedulerLifecycleRegistry {
  pub(crate) termination_state:    ArcShared<TerminationState>,
  pub(crate) root_started:         AtomicBool,
  pub(crate) scheduler_context:    SchedulerContext,
  pub(crate) tick_driver_snapshot: Option<TickDriverSnapshot>,
  pub(crate) tick_driver_bundle:   TickDriverBundle,
  pub(crate) tick_driver_stopper:  Option<Box<dyn TickDriverStopper>>,
  pub(crate) start_time:           Duration,
}

impl SchedulerLifecycleRegistry {
  pub(crate) fn new(scheduler_context: SchedulerContext, tick_driver_bundle: TickDriverBundle) -> Self {
    Self {
      termination_state: ArcShared::new(TerminationState::new()),
      root_started: AtomicBool::new(false),
      scheduler_context,
      tick_driver_snapshot: None,
      tick_driver_bundle,
      tick_driver_stopper: None,
      start_time: Duration::ZERO,
    }
  }
}
