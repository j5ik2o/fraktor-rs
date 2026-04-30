use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_actor_core_rs::core::kernel::system::Blocker;

use super::StdBlocker;

#[test]
fn default_returns_immediately_when_condition_is_already_true() {
  let blocker = StdBlocker::default();
  let calls = AtomicUsize::new(0);

  blocker.block_until(&|| {
    calls.fetch_add(1, Ordering::SeqCst);
    true
  });

  assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn poll_interval_is_clamped_and_condition_is_rechecked() {
  let blocker = StdBlocker::with_poll_interval(Duration::ZERO);
  let calls = AtomicUsize::new(0);

  blocker.block_until(&|| calls.fetch_add(1, Ordering::SeqCst) > 0);

  assert!(calls.load(Ordering::SeqCst) >= 2);
}
