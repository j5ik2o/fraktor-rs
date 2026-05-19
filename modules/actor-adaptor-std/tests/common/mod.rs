use std::{
  thread,
  time::{Duration, Instant},
};

pub fn wait_until(deadline_ms: u64, mut predicate: impl FnMut() -> bool) -> bool {
  let deadline = Instant::now() + Duration::from_millis(deadline_ms);
  while Instant::now() < deadline {
    if predicate() {
      return true;
    }
    thread::yield_now();
  }
  predicate()
}
