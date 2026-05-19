use fraktor_utils_core_rs::sync::SharedAccess;

use super::{AskFutures, MAX_TRACKED_ASK_FUTURES};
use crate::{
  actor::messaging::{AskError, AskResult},
  support::futures::{ActorFuture, ActorFutureShared},
};

fn pending_future() -> ActorFutureShared<AskResult> {
  ActorFutureShared::new(ActorFuture::new())
}

fn complete_timeout(future: &ActorFutureShared<AskResult>) {
  let waker = future.with_write(|inner| inner.complete(Err(AskError::Timeout)));
  assert!(waker.is_none());
}

fn registry_len(registry: &AskFutures) -> usize {
  registry.futures.len()
}

#[test]
fn push_prunes_ready_futures() {
  let mut registry = AskFutures::new();
  let ready = pending_future();
  complete_timeout(&ready);

  registry.push(ready);
  registry.push(pending_future());

  let drained = registry.drain_ready();
  assert_eq!(drained.len(), 0);
}

#[test]
fn push_limits_pending_futures() {
  let mut registry = AskFutures::new();
  for _ in 0..5000 {
    registry.push(pending_future());
  }

  assert_eq!(registry_len(&registry), 4096);
  assert!(registry.drain_ready().is_empty());
}

#[test]
fn drain_ready_removes_ready_futures() {
  let mut registry = AskFutures::new();
  let ready = pending_future();
  let pending = pending_future();

  registry.push(ready.clone());
  registry.push(pending.clone());
  complete_timeout(&ready);

  let drained = registry.drain_ready();
  assert_eq!(drained.len(), 1);
  assert_eq!(registry_len(&registry), 1);

  complete_timeout(&pending);
  assert_eq!(registry.drain_ready().len(), 1);
  assert_eq!(registry_len(&registry), 0);
}

#[test]
fn push_evicts_oldest_pending_future_first() {
  let mut registry = AskFutures::new();
  registry.push(pending_future());
  let second_oldest = pending_future();
  registry.push(second_oldest.clone());

  for _ in 2..MAX_TRACKED_ASK_FUTURES {
    registry.push(pending_future());
  }
  registry.push(pending_future());
  registry.push(pending_future());

  complete_timeout(&second_oldest);
  assert!(registry.drain_ready().is_empty());
  assert_eq!(registry_len(&registry), MAX_TRACKED_ASK_FUTURES);
}

#[test]
fn pruning_preserves_fifo_eviction_order() {
  let mut registry = AskFutures::new();
  registry.push(pending_future());
  let ready = pending_future();
  registry.push(ready.clone());
  let second_oldest = pending_future();
  registry.push(second_oldest.clone());

  for _ in 3..MAX_TRACKED_ASK_FUTURES {
    registry.push(pending_future());
  }
  complete_timeout(&ready);
  registry.push(pending_future());
  registry.push(pending_future());
  registry.push(pending_future());

  complete_timeout(&second_oldest);
  assert!(registry.drain_ready().is_empty());
  assert_eq!(registry_len(&registry), MAX_TRACKED_ASK_FUTURES);
}
