use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::*;
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
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
