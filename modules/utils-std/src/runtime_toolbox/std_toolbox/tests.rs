use fraktor_utils_core_rs::sync::RuntimeToolbox;

use super::StdToolbox;

#[test]
fn std_toolbox_is_debug() {
  let toolbox = StdToolbox::default();
  let debug_str = format!("{:?}", toolbox);
  assert!(debug_str.contains("StdToolbox"));
}

#[test]
fn std_toolbox_is_clone() {
  let toolbox1 = StdToolbox::default();
  let toolbox2 = toolbox1;
  assert_eq!(format!("{:?}", toolbox1), format!("{:?}", toolbox2));
}

#[test]
fn std_toolbox_default() {
  let toolbox = StdToolbox::default();
  let debug_str = format!("{:?}", toolbox);
  assert!(debug_str.contains("StdToolbox"));
}

#[test]
fn std_toolbox_implements_runtime_toolbox() {
  let _toolbox: StdToolbox = StdToolbox::default();
  fn assert_runtime_toolbox<T: RuntimeToolbox>() {}
  assert_runtime_toolbox::<StdToolbox>();
}
