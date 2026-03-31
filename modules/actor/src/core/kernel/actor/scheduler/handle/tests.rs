use super::SchedulerHandle;

// --- Phase 1 タスク6: cancel method on SchedulerHandle ---

/// A new handle starts in non-cancelled, non-completed state.
#[test]
fn new_handle_is_not_cancelled_and_not_completed() {
  let handle = SchedulerHandle::new(1);

  assert!(!handle.is_cancelled(), "new handle should not be cancelled");
  assert!(!handle.is_completed(), "new handle should not be completed");
}

/// `cancel` on a pending (not yet scheduled) handle returns `false`
/// because `try_cancel` transitions from Scheduled, not Pending.
#[test]
fn cancel_on_pending_handle_returns_false() {
  let handle = SchedulerHandle::new(1);

  let result = handle.cancel();
  assert!(!result, "cancel should fail on pending (not scheduled) handle");
  assert!(!handle.is_cancelled(), "handle should remain in pending state");
}

/// `cancel` on a scheduled handle returns `true` and marks it as cancelled.
#[test]
fn cancel_on_scheduled_handle_succeeds() {
  let handle = SchedulerHandle::new(2);
  handle.entry().mark_scheduled();

  let result = handle.cancel();
  assert!(result, "cancel should succeed on scheduled handle");
  assert!(handle.is_cancelled(), "handle should be cancelled after cancel()");
}

/// `cancel` on an already cancelled handle returns `false`.
#[test]
fn cancel_on_already_cancelled_handle_returns_false() {
  let handle = SchedulerHandle::new(3);
  handle.entry().mark_scheduled();
  let _ = handle.cancel();

  let result = handle.cancel();
  assert!(!result, "second cancel should return false");
  assert!(handle.is_cancelled(), "handle should still be cancelled");
}

/// `cancel` on a completed handle returns `false`.
#[test]
fn cancel_on_completed_handle_returns_false() {
  let handle = SchedulerHandle::new(4);
  handle.entry().mark_scheduled();
  handle.entry().try_begin_execute();
  handle.entry().mark_completed();

  let result = handle.cancel();
  assert!(!result, "cancel should fail on completed handle");
  assert!(!handle.is_cancelled(), "completed handle is not cancelled");
  assert!(handle.is_completed(), "handle should remain completed");
}

/// `cancel` on an executing handle returns `false`.
#[test]
fn cancel_on_executing_handle_returns_false() {
  let handle = SchedulerHandle::new(5);
  handle.entry().mark_scheduled();
  handle.entry().try_begin_execute();

  let result = handle.cancel();
  assert!(!result, "cancel should fail on executing handle");
  assert!(!handle.is_cancelled(), "executing handle should not be marked cancelled");
}

/// `is_cancelled` returns `true` after successful `cancel`.
#[test]
fn is_cancelled_reflects_cancel_state() {
  let handle = SchedulerHandle::new(6);
  assert!(!handle.is_cancelled());

  handle.entry().mark_scheduled();
  assert!(!handle.is_cancelled());

  let _ = handle.cancel();
  assert!(handle.is_cancelled());
}
