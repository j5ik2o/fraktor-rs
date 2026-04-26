//! Periodic heartbeat tick loop for the [`crate::std::watcher_actor::WatcherActor`].

use core::time::Duration;
use std::time::Instant;

use fraktor_remote_core_rs::domain::watcher::WatcherCommand;

use crate::std::watcher_actor::watcher_actor_handle::WatcherActorHandle;

/// Drives the watcher actor with periodic
/// [`WatcherCommand::HeartbeatTick`] messages.
///
/// `now_ms` is computed from `Instant::now().elapsed()` against an `epoch`
/// captured at the loop's start. This guarantees the watcher only ever
/// observes monotonic millis even if the wall clock jumps.
///
/// The loop terminates when the actor handle drops or the receiver is
/// closed. Returns the number of ticks that were successfully delivered.
pub async fn run_heartbeat_loop(handle: WatcherActorHandle, interval: Duration) -> u64 {
  let epoch = Instant::now();
  let mut delivered: u64 = 0;
  let mut ticker = tokio::time::interval(interval);
  // Skip the immediate fire-on-create tick — start at the first real interval.
  ticker.tick().await;
  loop {
    ticker.tick().await;
    let now_ms = epoch.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    if handle.submit(WatcherCommand::HeartbeatTick { now: now_ms }).is_err() {
      return delivered;
    }
    delivered = delivered.saturating_add(1);
  }
}
