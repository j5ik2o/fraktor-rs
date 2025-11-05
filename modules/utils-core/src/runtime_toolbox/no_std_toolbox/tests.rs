use super::*;

#[test]
fn test_no_std_toolbox_default() {
  let _toolbox = NoStdToolbox;
}

#[test]
fn test_no_std_toolbox_clone() {
  let toolbox = NoStdToolbox;
  let _cloned = toolbox;
}

#[test]
fn test_no_std_toolbox_debug() {
  let toolbox = NoStdToolbox;
  let debug_str = format!("{:?}", toolbox);
  assert_eq!(debug_str, "NoStdToolbox");
}
