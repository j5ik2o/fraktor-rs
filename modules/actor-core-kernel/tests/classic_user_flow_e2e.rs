#![cfg(not(target_os = "none"))]

mod common;

use std::vec::Vec;

use common::wait_until;
use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView, AskResult},
    props::Props,
    setup::ActorSystemConfig,
  },
  support::futures::ActorFutureShared,
  system::{ActorSystem, SpinBlocker},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

struct Start;
struct TellValue(u32);
struct AskValue;
struct ReplyValue(u32);
struct AfterStopProbe;

struct Worker {
  tell_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl Worker {
  fn new(tell_log: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { tell_log }
  }
}

impl Actor for Worker {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<TellValue>() {
      self.tell_log.lock().push(value.0);
    }
    if message.downcast_ref::<AskValue>().is_some() {
      let Some(sender) = message.sender() else {
        return Err(ActorError::recoverable("ask sender missing"));
      };
      let mut reply_to = sender.clone();
      reply_to
        .try_tell(AnyMessage::new(ReplyValue(42)))
        .map_err(|error| ActorError::recoverable(format!("classic E2E ask reply delivery failed: {error:?}")))?;
    }
    Ok(())
  }
}

struct Guardian {
  tell_log:       ArcShared<SpinSyncMutex<Vec<u32>>>,
  child_slot:     ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  ask_future:     ArcShared<SpinSyncMutex<Option<ActorFutureShared<AskResult>>>>,
  terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
}

impl Guardian {
  fn new(
    tell_log: ArcShared<SpinSyncMutex<Vec<u32>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    ask_future: ArcShared<SpinSyncMutex<Option<ActorFutureShared<AskResult>>>>,
    terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
  ) -> Self {
    Self { tell_log, child_slot, ask_future, terminated_log }
  }
}

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let tell_log = self.tell_log.clone();
      let mut child = ctx
        .spawn_child(&Props::from_fn(move || Worker::new(tell_log.clone())).with_name("classic-e2e-worker"))
        .map_err(|error| ActorError::recoverable(format!("classic E2E named spawn failed: {error:?}")))?;
      ctx
        .watch(child.actor_ref())
        .map_err(|error| ActorError::recoverable(format!("classic E2E watch failed: {error:?}")))?;
      child
        .try_tell(AnyMessage::new(TellValue(7)))
        .map_err(|error| ActorError::recoverable(format!("classic E2E tell delivery failed: {error:?}")))?;
      let ask = child.ask(AnyMessage::new(AskValue));
      self.ask_future.lock().replace(ask.future().clone());
      self.child_slot.lock().replace(child.clone());
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(terminated.value());
    Ok(())
  }
}

#[test]
fn classic_user_flow_observes_spawn_tell_ask_watch_stop_and_dead_letter() {
  let tell_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let ask_future = ArcShared::new(SpinSyncMutex::new(None));
  let terminated_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let tell_log = tell_log.clone();
    let child_slot = child_slot.clone();
    let ask_future = ask_future.clone();
    let terminated_log = terminated_log.clone();
    move || Guardian::new(tell_log.clone(), child_slot.clone(), ask_future.clone(), terminated_log.clone())
  });
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  assert!(wait_until(200, || {
    *tell_log.lock() == vec![7]
      && ask_future.lock().as_ref().map(|future| future.with_read(|inner| inner.is_ready())).unwrap_or(false)
      && child_slot.lock().is_some()
  }));

  let child = child_slot.lock().clone().expect("classic E2E child");
  let child_pid = child.pid();

  let ask_result = ask_future
    .lock()
    .as_ref()
    .expect("classic E2E ask future")
    .with_write(|future| future.try_take())
    .expect("classic E2E ask result")
    .expect("classic E2E ask response");
  let reply = ask_result.downcast_ref::<ReplyValue>().expect("classic E2E reply payload");
  assert_eq!(reply.0, 42);

  child.stop().expect("classic E2E stop");
  assert!(wait_until(200, || !terminated_log.lock().is_empty()));
  assert_eq!(*terminated_log.lock(), vec![child_pid.value()]);

  let mut stopped_child = child.clone();
  stopped_child.tell(AnyMessage::new(AfterStopProbe));
  assert!(wait_until(200, || {
    system.dead_letters().iter().any(|entry| {
      entry.recipient() == Some(child_pid)
        && entry.reason() == DeadLetterReason::RecipientUnavailable
        && entry.message().downcast_ref::<AfterStopProbe>().is_some()
    })
  }));

  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}
