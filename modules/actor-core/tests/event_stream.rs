#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{MailboxConfig, Props},
    scheduler::tick_driver::TestTickDriver,
    setup::ActorSystemConfig,
  },
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  system::{ActorSystem, SpinBlocker},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct NullActor;

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct TestGuardian {
  child_slot:  ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  child_props: Props,
}

impl TestGuardian {
  fn new(child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>, child_props: Props) -> Self {
    Self { child_slot, child_props }
  }
}

impl Actor for TestGuardian {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&self.child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
    *self.child_slot.lock() = Some(child);
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn dead_letter_event_is_published_when_send_fails() {
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));

  let mailbox_policy =
    MailboxPolicy::bounded(NonZeroUsize::new(1).expect("non-zero"), MailboxOverflowStrategy::DropNewest, None);
  let mailbox_config = MailboxConfig::new(mailbox_policy);
  let child_props = Props::from_fn(|| NullActor).with_mailbox_config(mailbox_config);

  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    let child_props = child_props.clone();
    move || TestGuardian::new(child_slot.clone(), child_props.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  wait_until(|| child_slot.lock().is_some());
  let mut child = child_slot.lock().clone().expect("child");

  child.suspend().expect("suspend child");
  // MB-H1 (Pekko parity): サスペンド中もエンキュー自体は受理される。
  // DeadLetters へ流すには容量超過 (DropNewest) を発生させる必要がある。
  // 容量 1 の bounded mailbox に 2 通送り、2 通目が DropNewest で弾かれて
  // DeadLetters に流れることを検証する。
  child.tell(AnyMessage::new("ping-1"));
  child.tell(AnyMessage::new("ping-2"));

  wait_until(|| !system.dead_letters().is_empty());
  let entries = system.dead_letters();
  assert!(!entries.is_empty());

  // Regression: reject された `ping-2` が dead-letter sink に 1 回だけ届くことを保証する。
  // MB-H3 では mailbox 層が overflow (MailboxFull) の唯一の DL 記録元になり
  // `EnqueueOutcome::Rejected` として成功扱いで返すため、`ActorRef::try_tell`
  // 側で `record_send_error` が二重に呼ばれることはない (Pekko 完全準拠)。
  let ping2_entries =
    entries.iter().filter(|entry| entry.message().downcast_ref::<&str>().copied() == Some("ping-2")).count();
  assert_eq!(ping2_entries, 1, "DropNewest-rejected envelope must be recorded once (got {ping2_entries}): {entries:?}");

  wait_until(|| events.lock().iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_))));

  child.resume().expect("resume child");
  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}

fn wait_until(condition: impl Fn() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}
