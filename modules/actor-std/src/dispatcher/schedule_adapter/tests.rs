use std::sync::atomic::Ordering;

use fraktor_actor_core_rs::{
  dispatcher::DispatcherGeneric,
  mailbox::{MailboxGeneric, MailboxPolicy},
};
use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;

impl StdScheduleAdapter {
  fn pending_calls(&self) -> usize {
    self.pending_calls.load(Ordering::Relaxed)
  }

  fn rejected_calls(&self) -> usize {
    self.rejected_calls.load(Ordering::Relaxed)
  }
}

#[test]
fn std_schedule_adapter_tracks_signals() {
  let adapter = StdScheduleAdapter::default();
  adapter.on_pending();
  adapter.on_pending();
  adapter.notify_rejected(1);
  assert_eq!(adapter.pending_calls(), 2);
  assert_eq!(adapter.rejected_calls(), 1);
}

#[test]
fn std_schedule_adapter_creates_valid_waker() {
  let mailbox = ArcShared::new(MailboxGeneric::new(MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherGeneric::with_inline_executor(mailbox);
  let adapter = StdScheduleAdapter::default();
  let waker = adapter.create_waker(dispatcher);
  waker.wake_by_ref();
}
