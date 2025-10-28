#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::vec::Vec;

use super::ArcSharedRingQueue;
use crate::collections::queue::traits::QueueRw;

type Element = u32;

#[test]
fn offer_and_poll_round_trip() {
  let queue = ArcSharedRingQueue::<Element>::new(8).with_dynamic(false);
  for value in 0..4 {
    queue.offer(value).unwrap();
  }

  let mut collected: Vec<Element> = Vec::new();
  while let Some(value) = queue.poll().unwrap() {
    collected.push(value);
  }

  assert_eq!(collected, Vec::from([0, 1, 2, 3]));
}

#[test]
fn clone_shares_state() {
  let queue = ArcSharedRingQueue::<Element>::new(4);
  queue.offer(42).unwrap();
  let clone = queue.clone();
  assert_eq!(clone.poll().unwrap(), Some(42));
  assert!(queue.poll().unwrap().is_none());
}

#[test]
fn clean_up_clears_queue() {
  let queue = ArcSharedRingQueue::<Element>::new(2).with_dynamic(false);
  queue.offer(7).unwrap();
  queue.offer(8).unwrap();
  queue.clean_up();
  assert!(queue.poll().unwrap().is_none());
}
