//! Scheduler-backed cluster time helpers.

use fraktor_actor_core_kernel_rs::actor::scheduler::SchedulerShared;
use fraktor_utils_core_rs::sync::SharedAccess;

pub(super) fn scheduler_time_secs(scheduler: &SchedulerShared) -> u64 {
  let dump = scheduler.with_read(|inner| inner.dump());
  let nanos = dump.resolution().as_nanos().saturating_mul(u128::from(dump.current_tick()));
  u64::try_from(nanos / 1_000_000_000).unwrap_or(u64::MAX)
}
