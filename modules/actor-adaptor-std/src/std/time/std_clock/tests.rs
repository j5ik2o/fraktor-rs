use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::pattern::Clock;

use super::StdClock;

#[test]
fn now_returns_monotonic_instant() {
  let clock = StdClock;

  let earlier = clock.now();
  let later = clock.now();

  assert!(later >= earlier);
}

#[test]
fn elapsed_since_uses_std_instant_elapsed() {
  let clock = StdClock;
  let earlier = clock.now();

  let elapsed = clock.elapsed_since(earlier);

  assert!(elapsed >= Duration::ZERO);
}
