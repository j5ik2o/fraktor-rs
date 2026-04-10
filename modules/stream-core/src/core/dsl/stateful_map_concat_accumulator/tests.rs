use alloc::{vec, vec::Vec};

use super::StatefulMapConcatAccumulator;

struct Doubler;

impl StatefulMapConcatAccumulator<i32, i32> for Doubler {
  fn apply(&mut self, input: i32) -> Vec<i32> {
    vec![input, input * 2]
  }
}

struct CountingAccumulator {
  count: usize,
}

impl StatefulMapConcatAccumulator<i32, i32> for CountingAccumulator {
  fn apply(&mut self, input: i32) -> Vec<i32> {
    self.count += 1;
    vec![input]
  }

  fn on_complete(&mut self) -> Vec<i32> {
    vec![self.count as i32]
  }
}

#[test]
fn apply_should_transform_elements() {
  let mut acc = Doubler;
  assert_eq!(acc.apply(5), vec![5, 10]);
  assert_eq!(acc.apply(3), vec![3, 6]);
}

#[test]
fn default_on_complete_should_return_empty() {
  let mut acc = Doubler;
  assert!(acc.on_complete().is_empty());
}

#[test]
fn on_complete_should_emit_trailing_elements() {
  let mut acc = CountingAccumulator { count: 0 };
  let _ = acc.apply(10);
  let _ = acc.apply(20);
  let _ = acc.apply(30);
  let trailing = acc.on_complete();
  assert_eq!(trailing, vec![3]);
}
