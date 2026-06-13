use proptest::prelude::*;

use crate::ddata::{Flag, ReplicatedData};

fn flag(enabled: bool) -> Flag {
  if enabled { Flag::disabled().switch_on() } else { Flag::disabled() }
}

#[test]
fn disabled_is_initial_value() {
  assert!(!Flag::disabled().is_enabled());
}

#[test]
fn switch_on_enables_flag() {
  assert!(Flag::disabled().switch_on().is_enabled());
}

#[test]
fn merge_prefers_enabled_value() {
  let disabled = Flag::disabled();
  let enabled = disabled.switch_on();

  assert!(disabled.merge(&enabled).is_enabled());
  assert!(enabled.merge(&disabled).is_enabled());
}

proptest! {
  #[test]
  fn merge_is_commutative(left in any::<bool>(), right in any::<bool>()) {
    let a = flag(left);
    let b = flag(right);

    prop_assert_eq!(a.merge(&b), b.merge(&a));
  }

  #[test]
  fn merge_is_associative(a in any::<bool>(), b in any::<bool>(), c in any::<bool>()) {
    let left = flag(a);
    let middle = flag(b);
    let right = flag(c);

    prop_assert_eq!(left.merge(&middle.merge(&right)), left.merge(&middle).merge(&right));
  }

  #[test]
  fn merge_is_idempotent(enabled in any::<bool>()) {
    let value = flag(enabled);

    prop_assert_eq!(value.merge(&value), value);
  }
}
