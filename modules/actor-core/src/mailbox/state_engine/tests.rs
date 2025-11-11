use core::sync::atomic::Ordering;

use super::{MailboxStateEngine, ScheduleHints};

impl MailboxStateEngine {
  fn raw_state_for_test(&self) -> u32 {
    self.state.load(Ordering::Acquire)
  }
}

#[test]
fn request_schedule_only_triggers_once_until_idle() {
  let engine = MailboxStateEngine::new();
  assert_eq!(engine.raw_state_for_test(), 0);
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false, backpressure_active: false };

  let first = engine.request_schedule(hints);
  assert!(first, "state after first attempt: {:#x}", engine.raw_state_for_test());
  // Second request before idle should not schedule again but should flag for reschedule.
  assert!(!engine.request_schedule(hints));

  engine.set_running();
  assert!(!engine.request_schedule(hints));
  // After idle, pending reschedule should fire exactly once.
  assert!(engine.set_idle());
  assert!(engine.request_schedule(hints));
}

#[test]
fn suspend_and_resume_control_user_messages() {
  let engine = MailboxStateEngine::new();
  assert!(!engine.is_suspended());
  engine.suspend();
  assert!(engine.is_suspended());
  engine.resume();
  assert!(!engine.is_suspended());
}

#[test]
fn backpressure_hint_requests_schedule_when_not_suspended() {
  let engine = MailboxStateEngine::new();
  assert!(!engine.is_suspended());
  let hints = ScheduleHints { has_system_messages: false, has_user_messages: false, backpressure_active: true };

  assert!(engine.request_schedule(hints));
  engine.set_running();
  assert!(!engine.request_schedule(hints));
  assert!(engine.set_idle());

  engine.suspend();
  assert!(!engine.request_schedule(hints));
  engine.resume();
  assert!(engine.request_schedule(hints));
}
