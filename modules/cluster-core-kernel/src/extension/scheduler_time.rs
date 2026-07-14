//! Scheduler-backed cluster time helpers.

use fraktor_actor_core_kernel_rs::actor::scheduler::SchedulerShared;

pub(super) fn scheduler_time_secs(scheduler: &SchedulerShared) -> u64 {
  scheduler.current_time_secs()
}
