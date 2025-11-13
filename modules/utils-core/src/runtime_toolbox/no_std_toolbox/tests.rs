use super::*;

#[test]
fn test_no_std_toolbox_default() {
  let _toolbox = NoStdToolbox::default();
}

#[test]
fn test_no_std_toolbox_clone() {
  let toolbox = NoStdToolbox::default();
  let _cloned = toolbox;
}

#[test]
fn test_no_std_toolbox_debug() {
  let toolbox = NoStdToolbox::default();
  let debug_str = format!("{:?}", toolbox);
  assert!(debug_str.contains("NoStdToolbox"));
}

#[test]
fn tick_source_emits_manual_ticks() {
  let toolbox = NoStdToolbox::default();
  let handle = toolbox.tick_source();
  let lease = handle.lease();
  handle.inject_manual_ticks(3);
  let event = lease.try_pull().expect("event");
  assert_eq!(event.ticks(), 3);
  assert!(lease.try_pull().is_none());
}
