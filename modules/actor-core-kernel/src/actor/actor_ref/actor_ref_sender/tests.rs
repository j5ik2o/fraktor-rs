use alloc::format;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::*;
use crate::actor::{error::SendError, messaging::AnyMessage};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

struct CountingSender {
  sends: ArcShared<SpinSyncMutex<usize>>,
}

impl ActorRefSender for CountingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    *self.sends.lock() += 1;
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn trait_object_compile_check() {
  let mut sender = TestSender;
  assert!(sender.send(AnyMessage::new(1_u8)).is_ok());
}

#[test]
fn apply_outcome_runs_scheduled_task() {
  let observed = ArcShared::new(SpinSyncMutex::new(false));
  let scheduled = {
    let observed = observed.clone();
    SendOutcome::Schedule(Box::new(move || {
      *observed.lock() = true;
    }))
  };

  let mut sender = TestSender;
  sender.apply_outcome(scheduled);

  assert!(*observed.lock());
}

#[test]
fn apply_outcome_accepts_delivered_without_side_effect() {
  let sends = ArcShared::new(SpinSyncMutex::new(0_usize));
  let mut sender = CountingSender { sends: sends.clone() };

  sender.apply_outcome(SendOutcome::Delivered);

  assert_eq!(*sends.lock(), 0);
}

#[test]
fn send_outcome_debug_is_stable_public_diagnostic() {
  let scheduled = SendOutcome::Schedule(Box::new(|| {}));

  assert!(format!("{:?}", SendOutcome::Delivered).contains("Delivered"));
  assert!(format!("{scheduled:?}").contains("Schedule"));
}
