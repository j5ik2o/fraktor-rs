use core::{num::NonZeroUsize, time::Duration};
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{MailboxConfig, Props},
    setup::ActorSystemConfig,
  },
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct Record(u32);

struct MailboxActor {
  records: SharedLock<Vec<u32>>,
}

impl Actor for MailboxActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(record) = message.downcast_ref::<Record>() {
      self.records.with_lock(|records| records.push(record.0));
    }
    Ok(())
  }
}

fn main() {
  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let capacity = NonZeroUsize::new(8).expect("positive capacity");
  let warn_threshold = NonZeroUsize::new(4).expect("positive threshold");
  let mailbox = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_warn_threshold(Some(warn_threshold));
  let props = Props::from_fn({
    let records = records.clone();
    move || MailboxActor { records: records.clone() }
  })
  .with_mailbox_config(mailbox);
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  for value in [1_u32, 2, 3] {
    system.user_guardian_ref().tell(AnyMessage::new(Record(value)));
  }
  wait_until(|| records.with_lock(|records| records.len() == 3));
  let snapshot = records.with_lock(|records| records.clone());
  assert_eq!(snapshot, vec![1, 2, 3]);
  println!("kernel_mailboxes delivered records: {snapshot:?}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
