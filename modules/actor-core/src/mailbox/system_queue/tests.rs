use crate::{actor_prim::Pid, mailbox::system_queue::SystemQueue, messaging::SystemMessage};

#[test]
fn fifo_ordering_is_preserved() {
  let queue = SystemQueue::new();
  queue.push(SystemMessage::Suspend);
  queue.push(SystemMessage::Resume);
  queue.push(SystemMessage::Stop);

  assert!(matches!(queue.pop(), Some(SystemMessage::Suspend)));
  assert!(matches!(queue.pop(), Some(SystemMessage::Resume)));
  assert!(matches!(queue.pop(), Some(SystemMessage::Stop)));
  assert!(queue.pop().is_none());
}

#[test]
fn len_tracks_push_and_pop_operations() {
  let queue = SystemQueue::new();
  assert_eq!(queue.len(), 0);

  queue.push(SystemMessage::Watch(Pid::new(1, 0)));
  assert_eq!(queue.len(), 1);
  queue.push(SystemMessage::Unwatch(Pid::new(2, 0)));
  assert_eq!(queue.len(), 2);

  queue.pop();
  assert_eq!(queue.len(), 1);
  queue.pop();
  assert_eq!(queue.len(), 0);
}
