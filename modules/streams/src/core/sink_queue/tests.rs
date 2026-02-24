use super::SinkQueue;

#[test]
fn should_return_none_when_queue_is_empty() {
  let queue = SinkQueue::<i32>::new();
  assert!(queue.pull().is_none());
  assert!(queue.is_empty());
  assert_eq!(queue.len(), 0);
}

#[test]
fn should_pull_elements_in_fifo_order() {
  let queue = SinkQueue::<i32>::new();
  queue.push(1);
  queue.push(2);
  queue.push(3);

  assert_eq!(queue.len(), 3);
  assert_eq!(queue.pull(), Some(1));
  assert_eq!(queue.pull(), Some(2));
  assert_eq!(queue.pull(), Some(3));
  assert!(queue.pull().is_none());
}

#[test]
fn should_share_state_across_clones() {
  let queue = SinkQueue::<i32>::new();
  let clone = queue.clone();

  queue.push(42);
  assert_eq!(clone.pull(), Some(42));
  assert!(queue.is_empty());
}

#[test]
fn should_report_length_correctly() {
  let queue = SinkQueue::<i32>::new();
  assert_eq!(queue.len(), 0);
  assert!(queue.is_empty());

  queue.push(10);
  assert_eq!(queue.len(), 1);
  assert!(!queue.is_empty());

  let _ = queue.pull();
  assert_eq!(queue.len(), 0);
  assert!(queue.is_empty());
}
