#![cfg(not(target_os = "none"))]

mod common;

use std::vec::Vec;

use common::wait_until;
use fraktor_actor_adaptor_std_rs::std::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    actor_ref::dead_letter::DeadLetterReason,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::{ActorSystem, SpinBlocker},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

struct Start;
struct Deliver(u32);
struct StopChild;
struct AfterStopProbe;

struct Worker {
  deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl Worker {
  fn new(deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { deliveries }
  }
}

impl Actor for Worker {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(deliver) = message.downcast_ref::<Deliver>() {
      self.deliveries.lock().push(deliver.0);
    }
    if message.downcast_ref::<StopChild>().is_some() {
      ctx.stop_self().map_err(|error| ActorError::recoverable(format!("stop self failed: {error:?}")))?;
    }
    Ok(())
  }
}

struct Guardian {
  deliveries:     ArcShared<SpinSyncMutex<Vec<u32>>>,
  child_slot:     ArcShared<SpinSyncMutex<Option<ChildRef>>>,
  terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
}

impl Guardian {
  fn new(
    deliveries: ArcShared<SpinSyncMutex<Vec<u32>>>,
    child_slot: ArcShared<SpinSyncMutex<Option<ChildRef>>>,
    terminated_log: ArcShared<SpinSyncMutex<Vec<u64>>>,
  ) -> Self {
    Self { deliveries, child_slot, terminated_log }
  }
}

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.log(LogLevel::Info, "cross-crate actor runtime boot");
      let deliveries = self.deliveries.clone();
      let mut child = ctx
        .spawn_child(&Props::from_fn(move || Worker::new(deliveries.clone())).with_name("cross-crate-worker"))
        .map_err(|error| ActorError::recoverable(format!("spawn child failed: {error:?}")))?;
      ctx.watch(child.actor_ref()).map_err(|error| ActorError::recoverable(format!("watch failed: {error:?}")))?;
      child
        .try_tell(AnyMessage::new(Deliver(7)))
        .map_err(|error| ActorError::recoverable(format!("tell failed: {error:?}")))?;
      self.child_slot.lock().replace(child);
    }
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, terminated: Pid) -> Result<(), ActorError> {
    self.terminated_log.lock().push(terminated.value());
    Ok(())
  }
}

struct LogRecorder {
  messages: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl LogRecorder {
  fn new(messages: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
    Self { messages }
  }
}

impl EventStreamSubscriber for LogRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Log(log) = event {
      self.messages.lock().push(log.message().to_owned());
    }
  }
}

const BOOT_TIMEOUT_MS: u64 = 2_000;

#[test]
fn actor_runtime_boot_wires_std_config_scheduler_logging_watch_stop_and_dead_letters() {
  let deliveries = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_slot = ArcShared::new(SpinSyncMutex::new(None));
  let terminated_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let log_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let config = std_actor_system_config(TestTickDriver::default());

  assert!(config.mailbox_clock().is_some(), "std config must install mailbox clock");

  let props = Props::from_fn({
    let deliveries = deliveries.clone();
    let child_slot = child_slot.clone();
    let terminated_log = terminated_log.clone();
    move || Guardian::new(deliveries.clone(), child_slot.clone(), terminated_log.clone())
  });
  let system = ActorSystem::create_from_props(&props, config).expect("actor system");
  let subscriber = subscriber_handle(LogRecorder::new(log_messages.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  assert!(system.tick_driver_snapshot().is_some(), "std config must provision scheduler tick driver");
  drop(system.scheduler());

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  assert!(wait_until(BOOT_TIMEOUT_MS, || {
    *deliveries.lock() == vec![7]
      && child_slot.lock().is_some()
      && log_messages.lock().iter().any(|message| message == "cross-crate actor runtime boot")
  }));

  let child = child_slot.lock().clone().expect("child should be spawned");
  let child_pid = child.pid();
  let mut child_ref = child.into_actor_ref();
  child_ref.tell(AnyMessage::new(StopChild));

  assert!(wait_until(BOOT_TIMEOUT_MS, || *terminated_log.lock() == vec![child_pid.value()]));

  child_ref.tell(AnyMessage::new(AfterStopProbe));
  assert!(wait_until(BOOT_TIMEOUT_MS, || {
    system.dead_letters().iter().any(|entry| {
      entry.recipient() == Some(child_pid)
        && entry.reason() == DeadLetterReason::RecipientUnavailable
        && entry.message().downcast_ref::<AfterStopProbe>().is_some()
    })
  }));

  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}
