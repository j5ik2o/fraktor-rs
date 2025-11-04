use core::time::Duration;

use super::TimerDeadline;

#[test]
fn timer_deadline_from_duration() {
  let duration = Duration::from_secs(5);
  let deadline = TimerDeadline::from_duration(duration);
  assert_eq!(deadline.as_duration(), duration);
}

#[test]
fn timer_deadline_as_duration() {
  let duration = Duration::from_millis(100);
  let deadline = TimerDeadline::from_duration(duration);
  assert_eq!(deadline.as_duration(), duration);
}

#[test]
fn timer_deadline_from_duration_trait() {
  let duration = Duration::from_secs(10);
  let deadline: TimerDeadline = duration.into();
  assert_eq!(deadline.as_duration(), duration);
}

#[test]
fn timer_deadline_into_duration() {
  let duration = Duration::from_nanos(1000);
  let deadline = TimerDeadline::from_duration(duration);
  let converted: Duration = deadline.into();
  assert_eq!(converted, duration);
}

#[test]
fn timer_deadline_clone() {
  let deadline1 = TimerDeadline::from_duration(Duration::from_secs(5));
  let deadline2 = deadline1;
  assert_eq!(deadline1, deadline2);
}

#[test]
fn timer_deadline_copy() {
  let deadline1 = TimerDeadline::from_duration(Duration::from_secs(3));
  let deadline2 = deadline1; // Copy trait???clone()??
  assert_eq!(deadline1, deadline2);
}

#[test]
fn timer_deadline_debug() {
  let deadline = TimerDeadline::from_duration(Duration::from_secs(7));
  let debug_str = format!("{:?}", deadline);
  assert!(debug_str.contains("TimerDeadline"));
}

#[test]
fn timer_deadline_partial_eq() {
  let deadline1 = TimerDeadline::from_duration(Duration::from_secs(5));
  let deadline2 = TimerDeadline::from_duration(Duration::from_secs(5));
  let deadline3 = TimerDeadline::from_duration(Duration::from_secs(10));
  assert_eq!(deadline1, deadline2);
  assert_ne!(deadline1, deadline3);
}

#[test]
fn timer_deadline_eq() {
  let deadline1 = TimerDeadline::from_duration(Duration::from_millis(100));
  let deadline2 = TimerDeadline::from_duration(Duration::from_millis(100));
  assert_eq!(deadline1, deadline2);
}
