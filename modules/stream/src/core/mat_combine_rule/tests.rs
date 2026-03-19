use crate::core::{KeepBoth, KeepLeft, KeepNone, KeepRight, MatCombineRule, StreamNotUsed};

fn combine<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}

#[test]
fn keep_left_returns_left() {
  let value: i32 = combine::<_, _, KeepLeft>(1, 2);
  assert_eq!(value, 1);
}

#[test]
fn keep_right_returns_right() {
  let value: i32 = combine::<_, _, KeepRight>(1, 2);
  assert_eq!(value, 2);
}

#[test]
fn keep_both_returns_pair() {
  let value: (i32, i32) = combine::<_, _, KeepBoth>(1, 2);
  assert_eq!(value, (1, 2));
}

#[test]
fn keep_none_returns_marker() {
  let _value: StreamNotUsed = combine::<_, _, KeepNone>(1, 2);
}
