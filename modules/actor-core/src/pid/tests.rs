use alloc::string::ToString;

use super::Pid;

#[test]
fn creates_and_formats_pid() {
  let pid = Pid::new(42, 7);
  assert_eq!(pid.value(), 42);
  assert_eq!(pid.generation(), 7);
  assert_eq!(pid.to_string(), "42:7");
}
