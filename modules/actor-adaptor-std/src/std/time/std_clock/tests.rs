use core::time::Duration;

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

#[test]
fn elapsed_since_uses_std_instant_elapsed() {
  let clock = StdClock;
  let first = clock.now();
  let second = clock.now();

  let elapsed = clock.elapsed_since(first);
  let upper_bound = second.duration_since(first) + Duration::from_millis(50);

  assert!(elapsed <= upper_bound);
}
