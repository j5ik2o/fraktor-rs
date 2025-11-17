use alloc::string::{String, ToString};

use super::WaitQueue;

#[test]
fn wait_queue_new() {
  let _queue: WaitQueue<&str> = WaitQueue::new();
}

#[test]
fn wait_queue_default() {
  let _queue: WaitQueue<i32> = WaitQueue::default();
}

#[test]
fn wait_queue_register_creates_waiter() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let wait_shared = queue.register().unwrap();
  drop(wait_shared);
}

#[test]
fn wait_queue_notify_success_completes_one() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let wait_shared = queue.register().unwrap();

  let notified = queue.notify_success();
  assert!(notified);

  drop(wait_shared);
}

#[test]
fn wait_queue_notify_success_no_waiters() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let notified = queue.notify_success();
  assert!(!notified);
}

#[test]
fn wait_queue_notify_success_multiple_waiters() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let _wait1 = queue.register().unwrap();
  let _wait2 = queue.register().unwrap();

  let notified1 = queue.notify_success();
  assert!(notified1);

  let notified2 = queue.notify_success();
  assert!(notified2);
}

#[test]
fn wait_queue_notify_error_all() {
  let mut queue: WaitQueue<String> = WaitQueue::new();
  let _wait1 = queue.register().unwrap();
  let _wait2 = queue.register().unwrap();

  queue.notify_error_all("error".to_string());
}

#[test]
fn wait_queue_notify_error_all_with() {
  let mut queue: WaitQueue<i32> = WaitQueue::new();
  let _wait1 = queue.register().unwrap();
  let _wait2 = queue.register().unwrap();

  let mut counter = 0;
  queue.notify_error_all_with(|| {
    counter += 1;
    counter
  });

  assert_eq!(counter, 2);
}

#[test]
fn wait_queue_notify_error_all_empty() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  queue.notify_error_all("error");
}

#[test]
fn wait_queue_notify_error_all_with_empty() {
  let mut queue: WaitQueue<i32> = WaitQueue::new();
  queue.notify_error_all_with(|| 42);
}
