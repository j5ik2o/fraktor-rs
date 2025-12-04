use super::*;
use crate::{
  core::{collections::queue::SyncQueue, sync::ArcShared},
  std::{collections::queue::StdSyncMpscQueueShared, sync_mutex::StdSyncMutex},
};
#[test]
fn offer_and_poll_roundtrip() {
  let mut backend = MpscBackend::new();

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.len(), 2);
  assert_eq!(backend.poll().unwrap(), 1);
  assert_eq!(backend.poll().unwrap(), 2);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn mpsc_backend_new() {
  let backend = MpscBackend::<i32>::new();
  assert_eq!(backend.capacity(), usize::MAX);
  assert_eq!(backend.overflow_policy(), OverflowPolicy::Grow);
  assert!(!backend.is_closed());
}

#[test]
fn overflow_policy_returns_grow() {
  let backend = MpscBackend::<i32>::new();
  assert_eq!(backend.overflow_policy(), OverflowPolicy::Grow);
}

#[test]
fn is_closed_returns_false_initially() {
  let backend = MpscBackend::<i32>::new();
  assert!(!backend.is_closed());
}

#[test]
fn capacity_returns_unbounded() {
  let backend = MpscBackend::<i32>::new();
  assert_eq!(backend.capacity(), usize::MAX);
}

#[test]
fn empty_queue_returns_empty_error() {
  let mut backend = MpscBackend::<i32>::new();
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn len_returns_correct_length() {
  let mut backend = MpscBackend::new();

  assert_eq!(backend.len(), 0);
  backend.offer(1).unwrap();
  assert_eq!(backend.len(), 1);
  backend.offer(2).unwrap();
  assert_eq!(backend.len(), 2);
  backend.poll().unwrap();
  assert_eq!(backend.len(), 1);
}

#[test]
fn is_empty_when_len_is_zero() {
  let mut backend = MpscBackend::new();

  assert_eq!(backend.len(), 0);
  backend.offer(1).unwrap();
  assert_ne!(backend.len(), 0);
  backend.poll().unwrap();
  assert_eq!(backend.len(), 0);
}

#[test]
fn disconnected_when_receiver_dropped() {
  let backend = MpscBackend::<i32>::new();
  let sender = backend.sender().clone();

  // Drop the entire backend (including receiver)
  drop(backend);

  // Now offer through the cloned sender should fail
  assert!(sender.send(1).is_err());
}

#[test]
fn disconnected_when_sender_dropped() {
  // Create a backend and immediately drop the sender
  let (sender, receiver) = mpsc::channel::<i32>();
  drop(sender);

  // Poll on disconnected channel should return Disconnected
  assert!(matches!(receiver.try_recv(), Err(mpsc::TryRecvError::Disconnected)));
}

#[test]
fn multiple_offers_and_polls() {
  let mut backend = MpscBackend::new();

  for i in 0..100 {
    backend.offer(i).unwrap();
  }

  assert_eq!(backend.len(), 100);

  for i in 0..100 {
    assert_eq!(backend.poll().unwrap(), i);
  }

  assert_eq!(backend.len(), 0);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn default_creates_new_backend() {
  let backend = MpscBackend::<i32>::default();
  assert_eq!(backend.capacity(), usize::MAX);
  assert_eq!(backend.len(), 0);
}

#[test]
fn works_with_sync_queue_shared() {
  use crate::core::{
    collections::queue::{SyncQueue, SyncQueueShared, type_keys::MpscKey},
    sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
  };

  // SyncQueueShared<T, MpscKey, MpscBackend<T>> が構築できることを確認
  let backend = MpscBackend::<i32>::new();
  let sync_queue = SyncQueue::<i32, MpscKey, MpscBackend<i32>>::new(backend);
  let mutex = SpinSyncMutex::new(sync_queue);
  let shared = ArcShared::new(mutex);
  let queue = SyncQueueShared::<i32, MpscKey, MpscBackend<i32>>::new(shared);

  // 基本的な操作ができることを確認
  queue.offer(1).unwrap();
  queue.offer(2).unwrap();
  assert_eq!(queue.len(), 2);
  assert_eq!(queue.poll().unwrap(), 1);
  assert_eq!(queue.poll().unwrap(), 2);
}

#[test]
fn works_with_std_sync_mpsc_queue_shared() {
  // StdSyncMpscQueueShared<T, MpscBackend<T>> の型エイリアスが使えることを確認
  let backend = MpscBackend::<i32>::new();
  let sync_queue = SyncQueue::new(backend);
  let mutex = StdSyncMutex::new(sync_queue);
  let shared = ArcShared::new(mutex);
  let queue = StdSyncMpscQueueShared::<i32, MpscBackend<i32>>::new(shared);

  // 基本的な操作ができることを確認
  queue.offer(10).unwrap();
  queue.offer(20).unwrap();
  queue.offer(30).unwrap();
  assert_eq!(queue.len(), 3);
  assert_eq!(queue.poll().unwrap(), 10);
  assert_eq!(queue.poll().unwrap(), 20);
  assert_eq!(queue.poll().unwrap(), 30);
}
