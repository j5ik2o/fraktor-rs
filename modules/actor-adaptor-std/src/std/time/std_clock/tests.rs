use fraktor_actor_core_rs::core::kernel::pattern::Clock;

use super::StdClock;

#[test]
fn now_returns_monotonic_instant() {
  let clock = StdClock;

  // StdClock::now is kept here as a Clock-trait compatibility check, even
  // though the concrete implementation delegates to std::time::Instant::now.
  let earlier = clock.now();
  let later = clock.now();

  assert!(later >= earlier);
}
