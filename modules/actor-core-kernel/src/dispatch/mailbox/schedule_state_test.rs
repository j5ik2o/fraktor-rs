use core::sync::atomic::Ordering;

use super::{super::ScheduleHints, CloseRequestOutcome, MailboxScheduleState, RunFinishOutcome};

impl MailboxScheduleState {
  fn raw_state_for_test(&self) -> u32 {
    self.state.load(Ordering::Acquire)
  }
}

#[test]
fn request_schedule_only_triggers_once_until_idle() {
  let schedule_state = MailboxScheduleState::new();
  assert_eq!(schedule_state.raw_state_for_test(), 0);
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false, backpressure_active: false };

  let first = schedule_state.request_schedule(hints);
  assert!(first, "state after first attempt: {:#x}", schedule_state.raw_state_for_test());
  // Second request before idle should not schedule again but should flag for reschedule.
  assert!(!schedule_state.request_schedule(hints));

  schedule_state.set_running();
  assert!(!schedule_state.request_schedule(hints));
  // After idle, pending reschedule should fire exactly once.
  assert!(matches!(schedule_state.finish_run(), RunFinishOutcome::Continue { pending_reschedule: true }));
  assert!(schedule_state.request_schedule(hints));
}

#[test]
fn suspend_and_resume_control_user_messages() {
  let schedule_state = MailboxScheduleState::new();
  assert!(!schedule_state.is_suspended());
  schedule_state.suspend();
  assert!(schedule_state.is_suspended());
  schedule_state.resume();
  assert!(!schedule_state.is_suspended());
}

#[test]
fn backpressure_hint_requests_schedule_when_not_suspended() {
  let schedule_state = MailboxScheduleState::new();
  assert!(!schedule_state.is_suspended());
  let hints = ScheduleHints { has_system_messages: false, has_user_messages: false, backpressure_active: true };

  assert!(schedule_state.request_schedule(hints));
  schedule_state.set_running();
  assert!(!schedule_state.request_schedule(hints));
  assert!(matches!(schedule_state.finish_run(), RunFinishOutcome::Continue { pending_reschedule: true }));

  schedule_state.suspend();
  assert!(!schedule_state.request_schedule(hints));
  schedule_state.resume();
  assert!(schedule_state.request_schedule(hints));
}

#[test]
fn backpressure_hint_is_ignored_while_suspended() {
  let schedule_state = MailboxScheduleState::new();
  let hints = ScheduleHints { has_system_messages: false, has_user_messages: false, backpressure_active: true };

  schedule_state.suspend();
  assert!(schedule_state.is_suspended());
  assert!(!schedule_state.request_schedule(hints));

  schedule_state.resume();
  assert!(schedule_state.request_schedule(hints));
  schedule_state.set_running();
  assert!(!schedule_state.request_schedule(hints));
  assert!(matches!(schedule_state.finish_run(), RunFinishOutcome::Continue { pending_reschedule: true }));
}

#[test]
fn idle_close_request_acquires_finalizer_immediately() {
  let schedule_state = MailboxScheduleState::new();

  assert_eq!(schedule_state.request_close(), CloseRequestOutcome::CallerOwnsFinalizer);
  assert!(schedule_state.is_close_requested());
  assert!(!schedule_state.request_schedule(ScheduleHints {
    has_system_messages: true,
    has_user_messages:   false,
    backpressure_active: false,
  }));
}

#[test]
fn running_close_request_defers_cleanup_to_runner() {
  let schedule_state = MailboxScheduleState::new();
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false, backpressure_active: false };

  assert!(schedule_state.request_schedule(hints));
  schedule_state.set_running();
  assert_eq!(schedule_state.request_close(), CloseRequestOutcome::RunnerOwnsFinalizer);
  assert!(matches!(schedule_state.finish_run(), RunFinishOutcome::FinalizeNow));
  schedule_state.finish_cleanup();
  assert!(schedule_state.is_cleanup_done());
}
