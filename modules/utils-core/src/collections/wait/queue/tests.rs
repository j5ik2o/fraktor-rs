use super::WaitQueue;

#[test]
fn wait_queue_new() {
  let queue: WaitQueue<&str> = WaitQueue::new();
  // ???????????
}

#[test]
fn wait_queue_default() {
  let queue: WaitQueue<i32> = WaitQueue::default();
  // Default?????????????????
}

#[test]
fn wait_queue_register_creates_waiter() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let wait_shared = queue.register();
  // register()?WaitShared????????
  drop(wait_shared);
}

#[test]
fn wait_queue_notify_success_completes_one() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let wait_shared = queue.register();

  // ????1?????????????
  let notified = queue.notify_success();
  assert!(notified);

  drop(wait_shared);
}

#[test]
fn wait_queue_notify_success_no_waiters() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  // ?????????
  let notified = queue.notify_success();
  assert!(!notified);
}

#[test]
fn wait_queue_notify_success_multiple_waiters() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  let _wait1 = queue.register();
  let _wait2 = queue.register();

  // ??????????????
  let notified1 = queue.notify_success();
  assert!(notified1);

  // 2???????????
  let notified2 = queue.notify_success();
  assert!(notified2);
}

#[test]
fn wait_queue_notify_error_all() {
  let mut queue: WaitQueue<String> = WaitQueue::new();
  let _wait1 = queue.register();
  let _wait2 = queue.register();

  queue.notify_error_all("error".to_string());

  // ????????????????????
}

#[test]
fn wait_queue_notify_error_all_with() {
  let mut queue: WaitQueue<i32> = WaitQueue::new();
  let _wait1 = queue.register();
  let _wait2 = queue.register();

  let mut counter = 0;
  queue.notify_error_all_with(|| {
    counter += 1;
    counter
  });

  // ??????2???????????????????????????
  assert_eq!(counter, 2);
}

#[test]
fn wait_queue_notify_error_all_empty() {
  let mut queue: WaitQueue<&str> = WaitQueue::new();
  // ???????????panic???
  queue.notify_error_all("error");
}

#[test]
fn wait_queue_notify_error_all_with_empty() {
  let mut queue: WaitQueue<i32> = WaitQueue::new();
  // ???????????panic???
  queue.notify_error_all_with(|| 42);
}
