use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::{
  sync::{Barrier, mpsc},
  thread,
};

use super::*;
use crate::core::kernel::{
  actor::{error::SendError, messaging::AnyMessage},
  system::lock_provider::SharedLock,
};

struct DeferredScheduleSender {
  send_count:             Arc<AtomicUsize>,
  first_schedule_entered: mpsc::Sender<()>,
  first_schedule_release: Arc<Barrier>,
}

impl ActorRefSender for DeferredScheduleSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    let send_index = self.send_count.fetch_add(1, Ordering::SeqCst);
    if send_index == 0 {
      let first_schedule_entered = self.first_schedule_entered.clone();
      let first_schedule_release = self.first_schedule_release.clone();
      return Ok(SendOutcome::Schedule(Box::new(move || {
        first_schedule_entered.send(()).expect("first schedule should notify the test");
        first_schedule_release.wait();
      })));
    }

    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn debug_sender_allows_parallel_send_after_releasing_inner_lock() {
  let (first_schedule_entered_tx, first_schedule_entered_rx) = mpsc::channel();
  let first_schedule_release = Arc::new(Barrier::new(2));
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::debug(
    Box::new(DeferredScheduleSender {
      send_count:             Arc::new(AtomicUsize::new(0)),
      first_schedule_entered: first_schedule_entered_tx,
      first_schedule_release: first_schedule_release.clone(),
    }),
    "actor_ref_sender_shared.inner",
  ));

  let mut first_sender = sender.clone();
  let first_handle = thread::spawn(move || first_sender.send(AnyMessage::new(1_u8)));

  first_schedule_entered_rx.recv().expect("first send should enter deferred schedule");

  let mut second_sender = sender.clone();
  let second_handle = thread::spawn(move || second_sender.send(AnyMessage::new(2_u8)));
  let second_join = second_handle.join();

  // 1本目はロック外の後処理で待機しているだけなので、2本目は panic せず通るべき。
  first_schedule_release.wait();

  let first_result = first_handle.join().expect("first send thread should not panic");
  let second_result = second_join.expect("second send thread should not panic");

  assert!(first_result.is_ok(), "first send should succeed");
  assert!(second_result.is_ok(), "second send should succeed without false nested-send detection");
}
