//! RAII token for an [`ExecutorShared`] drain-owner claim.
//!
//! See [`ExecutorShared::enter_drive_guard`](super::ExecutorShared::enter_drive_guard)
//! for the acquisition path. This token exists purely so the release path is
//! bound to `Drop`; there is no public `exit_drive_guard` method.

#[cfg(test)]
mod tests;

use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::core::sync::ArcShared;

/// RAII handle that releases an [`ExecutorShared`] drain-owner claim when
/// dropped.
///
/// The token is produced by
/// [`ExecutorShared::enter_drive_guard`](super::ExecutorShared::enter_drive_guard).
/// If the CAS succeeded, `claimed` is `true` and `Drop` stores `false` back
/// into `running`, allowing the next caller to become the drain owner. If the
/// CAS failed — another drain owner was already active — `claimed` is `false`
/// and `Drop` is a no-op so the outer owner keeps running the drain loop.
///
/// This type must be kept alive for the full guarded region; dropping it
/// immediately (`let _ = executor.enter_drive_guard()`) would release the
/// claim before any guarded work runs, defeating the purpose of the guard.
/// The `#[must_use]` attribute promotes such misuse into a compile-time
/// warning (which project-wide lint settings elevate to an error).
///
/// # Tail drain is intentionally **not** performed in `Drop`
///
/// Tasks queued into `trampoline.pending` while the guard was held remain
/// there after the token drops. A subsequent
/// [`ExecutorShared::execute`](super::ExecutorShared::execute) call naturally
/// picks them up via the existing CAS-based drain-owner selection. Performing
/// a synchronous tail drain inside `Drop` would defeat the entire point of
/// the guard (child mailboxes would run on the caller's stack again — the
/// very reentrance this mechanism exists to prevent), so that path is
/// forbidden here.
#[must_use = "DriveGuardToken must be held for the full guarded region; \
              drop it at the end of the scope where `enter_drive_guard` was called"]
pub(crate) struct DriveGuardToken {
  claimed: bool,
  running: ArcShared<AtomicBool>,
}

impl DriveGuardToken {
  /// Creates a token with the given claim outcome. Only
  /// [`ExecutorShared::enter_drive_guard`](super::ExecutorShared::enter_drive_guard)
  /// constructs tokens; callers outside this crate cannot fabricate one because
  /// both the struct and its constructor are `pub(crate)`.
  pub(crate) const fn new(claimed: bool, running: ArcShared<AtomicBool>) -> Self {
    Self { claimed, running }
  }
}

impl Drop for DriveGuardToken {
  fn drop(&mut self) {
    if self.claimed {
      // 禁止事項: ここでトランポリンの pending キューを末尾消費してはならない
      // （上の型レベルドキュメント参照）。許可されるのはクレームの解放のみ。
      self.running.store(false, Ordering::Release);
    }
  }
}
