use crate::core::{
  dsl::DelayStrategy,
  restart::{FixedDelay, LinearIncreasingDelay},
};

#[test]
fn fixed_delay_returns_constant() {
  let mut strategy = FixedDelay::new(5);
  assert_eq!(strategy.next_delay(&42), 5);
  assert_eq!(strategy.next_delay(&99), 5);
  assert_eq!(strategy.next_delay(&0), 5);
}

#[test]
fn fixed_delay_zero_passes_immediately() {
  let mut strategy = FixedDelay::new(0);
  assert_eq!(strategy.next_delay(&1), 0);
}

#[test]
fn linear_increasing_delay_increases_on_true() {
  let mut strategy = LinearIncreasingDelay::new(
    10,             // increase_step
    |_: &i32| true, // always increase
    0,              // initial_delay
    50,             // max_delay
  );
  assert_eq!(strategy.next_delay(&1), 10);
  assert_eq!(strategy.next_delay(&2), 20);
  assert_eq!(strategy.next_delay(&3), 30);
  assert_eq!(strategy.next_delay(&4), 40);
  assert_eq!(strategy.next_delay(&5), 50);
  // Should cap at max_delay
  assert_eq!(strategy.next_delay(&6), 50);
}

#[test]
fn linear_increasing_delay_resets_on_false() {
  let mut strategy = LinearIncreasingDelay::new(10, |x: &i32| *x > 0, 0, 100);
  assert_eq!(strategy.next_delay(&1), 10);
  assert_eq!(strategy.next_delay(&1), 20);
  // Reset
  assert_eq!(strategy.next_delay(&0), 0);
  // Start increasing again
  assert_eq!(strategy.next_delay(&1), 10);
}

#[test]
fn linear_increasing_delay_with_initial_delay() {
  let mut strategy = LinearIncreasingDelay::new(5, |_: &i32| true, 10, 30);
  assert_eq!(strategy.next_delay(&1), 15);
  assert_eq!(strategy.next_delay(&2), 20);
  assert_eq!(strategy.next_delay(&3), 25);
  assert_eq!(strategy.next_delay(&4), 30);
  assert_eq!(strategy.next_delay(&5), 30);
}

#[test]
#[should_panic(expected = "increase_step must be positive")]
fn linear_increasing_delay_rejects_zero_step() {
  let _ = LinearIncreasingDelay::new(0, |_: &i32| true, 0, 10);
}

#[test]
#[should_panic(expected = "max_delay must be greater than initial_delay")]
fn linear_increasing_delay_rejects_invalid_max() {
  let _ = LinearIncreasingDelay::new(5, |_: &i32| true, 10, 10);
}
