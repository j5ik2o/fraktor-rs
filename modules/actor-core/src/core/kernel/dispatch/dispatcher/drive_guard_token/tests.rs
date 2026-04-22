use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::DriveGuardToken;

/// Test-only extension exposing the `claimed` field without widening
/// production visibility.
impl DriveGuardToken {
  pub(crate) fn claimed(&self) -> bool {
    self.claimed
  }
}

#[test]
fn drop_stores_false_when_claimed() {
  let running = ArcShared::new(AtomicBool::new(true));
  {
    let token = DriveGuardToken::new(true, running.clone());
    assert!(token.claimed());
    assert!(running.load(Ordering::Acquire));
  }
  assert!(!running.load(Ordering::Acquire), "drop must release running when claimed=true");
}

#[test]
fn drop_is_noop_when_not_claimed() {
  let running = ArcShared::new(AtomicBool::new(true));
  {
    let token = DriveGuardToken::new(false, running.clone());
    assert!(!token.claimed());
  }
  assert!(running.load(Ordering::Acquire), "drop must not touch running when claimed=false");
}
