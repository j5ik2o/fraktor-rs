use super::DefaultResizer;
use crate::core::typed::routing::Resizer;

#[test]
fn default_resizer_getters_return_configured_values() {
  let resizer = DefaultResizer::new(2, 8, 10);
  assert_eq!(resizer.lower_bound(), 2);
  assert_eq!(resizer.upper_bound(), 8);
  assert_eq!(resizer.messages_per_resize(), 10);
}

#[test]
#[should_panic(expected = "lower_bound must be positive")]
fn default_resizer_rejects_zero_lower_bound() {
  let _ = DefaultResizer::new(0, 5, 1);
}

#[test]
#[should_panic(expected = "upper_bound must be >= lower_bound")]
fn default_resizer_rejects_upper_less_than_lower() {
  let _ = DefaultResizer::new(5, 3, 1);
}

#[test]
#[should_panic(expected = "messages_per_resize must be positive")]
fn default_resizer_rejects_zero_messages_per_resize() {
  let _ = DefaultResizer::new(2, 5, 0);
}

#[test]
fn is_time_for_resize_returns_true_on_modulo_boundary() {
  let resizer = DefaultResizer::new(1, 10, 5);
  // カウンタ0は初回メッセージのため、不要なリサイズを防ぐ
  assert!(!resizer.is_time_for_resize(0));
  assert!(!resizer.is_time_for_resize(1));
  assert!(!resizer.is_time_for_resize(4));
  assert!(resizer.is_time_for_resize(5));
  assert!(resizer.is_time_for_resize(10));
}

#[test]
fn is_time_for_resize_with_period_one_returns_true_except_zero() {
  let resizer = DefaultResizer::new(1, 10, 1);
  // カウンタ0は初回メッセージのため false
  assert!(!resizer.is_time_for_resize(0));
  for counter in 1..10 {
    assert!(resizer.is_time_for_resize(counter));
  }
}

#[test]
fn is_time_for_resize_rejects_zero_counter() {
  // 0u64.is_multiple_of(n) は全ての n で true を返すため、
  // 初回メッセージでの不要なリサイズチェックを防ぐ回帰テスト
  let resizer = DefaultResizer::new(1, 10, 100);
  assert!(!resizer.is_time_for_resize(0), "カウンタ0でリサイズが発火してはならない");
}

#[test]
fn resize_returns_positive_delta_when_below_lower_bound() {
  let resizer = DefaultResizer::new(3, 10, 1);
  assert_eq!(resizer.resize(1), 2);
  assert_eq!(resizer.resize(2), 1);
}

#[test]
fn resize_returns_negative_delta_when_above_upper_bound() {
  let resizer = DefaultResizer::new(2, 5, 1);
  assert_eq!(resizer.resize(7), -2);
  assert_eq!(resizer.resize(6), -1);
}

#[test]
fn resize_returns_zero_when_within_bounds() {
  let resizer = DefaultResizer::new(2, 5, 1);
  assert_eq!(resizer.resize(2), 0);
  assert_eq!(resizer.resize(3), 0);
  assert_eq!(resizer.resize(5), 0);
}

#[test]
fn resize_returns_zero_at_exact_bounds() {
  let resizer = DefaultResizer::new(3, 3, 1);
  assert_eq!(resizer.resize(3), 0);
}
