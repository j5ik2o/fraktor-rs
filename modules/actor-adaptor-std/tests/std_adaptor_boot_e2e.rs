#![cfg(not(target_os = "none"))]

mod common;

use std::vec::Vec;

use common::wait_until;
use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
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
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

struct Start;

struct BootActor {
  message_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl BootActor {
  fn new(message_log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { message_log }
  }
}

impl Actor for BootActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      self.message_log.lock().push("start");
      ctx.log(LogLevel::Info, "std adaptor boot e2e");
      ctx.stop_self().expect("stop_self should succeed in std adaptor boot E2E");
    }
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

const STD_ADAPTOR_BOOT_TIMEOUT_MS: u64 = 2_000;

#[test]
fn std_adaptor_boot_flow_wires_config_dispatcher_mailbox_scheduler_logging_and_termination() {
  let message_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let log_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let config = std_actor_system_config(TestTickDriver::default());

  assert!(config.mailbox_clock().is_some(), "std config must install mailbox clock");

  let props = Props::from_fn({
    let message_log = message_log.clone();
    move || BootActor::new(message_log.clone())
  });
  let system = ActorSystem::create_from_props(&props, config).expect("std actor system");
  let subscriber = subscriber_handle(LogRecorder::new(log_messages.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  assert!(system.tick_driver_snapshot().is_some(), "std config must provision scheduler tick driver");
  // boot 後に scheduler accessor が panic せず取得できることを smoke check する。
  let _ = system.scheduler();

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  assert!(wait_until(STD_ADAPTOR_BOOT_TIMEOUT_MS, || {
    *message_log.lock() == vec!["start"] && log_messages.lock().iter().any(|message| message == "std adaptor boot e2e")
  }));

  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}
