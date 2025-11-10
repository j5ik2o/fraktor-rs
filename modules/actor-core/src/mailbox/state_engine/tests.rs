use super::{MailboxStateEngine, ScheduleHints};

#[test]
fn request_schedule_only_triggers_once_until_idle() {
  let engine = MailboxStateEngine::new();
  assert_eq!(engine.raw_state(), 0);
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false };

  let first = engine.request_schedule(hints);
  assert!(first, "state after first attempt: {:#x}", engine.raw_state());
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
