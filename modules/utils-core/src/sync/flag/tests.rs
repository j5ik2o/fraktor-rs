#[test]
fn flag_new_creates_with_initial_value() {
  let flag_true = super::Flag::new(true);
  assert!(flag_true.get());

  let flag_false = super::Flag::new(false);
  assert!(!flag_false.get());
}

#[test]
fn flag_set_changes_value() {
  let flag = super::Flag::new(false);
  assert!(!flag.get());

  flag.set(true);
  assert!(flag.get());

  flag.set(false);
  assert!(!flag.get());
}

#[test]
fn flag_clear_sets_to_false() {
  let flag = super::Flag::new(true);
  assert!(flag.get());

  flag.clear();
  assert!(!flag.get());
}

#[test]
fn flag_default_is_false() {
  let flag = super::Flag::default();
  assert!(!flag.get());
}

#[test]
fn flag_clone_works() {
  let flag1 = super::Flag::new(true);
  let flag2 = flag1.clone();

  assert!(flag1.get());
  assert!(flag2.get());

  // フラグは共有されているので、片方を変更すると両方に影響する
  flag1.set(false);
  assert!(!flag1.get());
  assert!(!flag2.get());
}

#[test]
fn flag_debug_format() {
  let flag = super::Flag::new(true);
  let debug_str = format!("{:?}", flag);
  assert!(debug_str.contains("Flag"));
  assert!(debug_str.contains("true"));
}
