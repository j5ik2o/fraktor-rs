use std::{
  thread,
  time::{Duration, Instant},
};

// actor-adaptor-std/tests/common/mod.rs と同じ形を保つ。3 か所目が必要に
// なった時点で共通 test utility crate を検討する。deadline 後の最終評価は、
// 期限直後に条件が成立する race を緩和するために意図して残している。
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
