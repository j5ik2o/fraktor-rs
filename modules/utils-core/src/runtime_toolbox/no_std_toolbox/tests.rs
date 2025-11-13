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
