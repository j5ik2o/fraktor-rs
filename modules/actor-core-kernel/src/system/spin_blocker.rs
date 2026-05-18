//! Spin-loop blocker for test and no_std environments.

use super::blocker::Blocker;

/// Spin-loop blocker for test and no_std environments.
///
/// **Not recommended for production use.** Prefer platform-specific
/// implementations such as `StdBlocker` from the adapter crate.
pub struct SpinBlocker;

impl Blocker for SpinBlocker {
  fn block_until(&self, condition: &dyn Fn() -> bool) {
    while !condition() {
      core::hint::spin_loop();
    }
  }
}
