use core::time::Duration;

use super::TimerDeadLine;

#[test]
fn duration_round_trip() {
  let duration = Duration::from_millis(150);
  let deadline = TimerDeadLine::from(duration);
  assert_eq!(deadline.as_duration(), duration);
  let back: Duration = deadline.into();
  assert_eq!(back, duration);
}
