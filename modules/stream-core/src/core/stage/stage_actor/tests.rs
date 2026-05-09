use alloc::{boxed::Box, vec, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::StageActor;
use crate::core::{
  StreamError,
  stage::{StageActorEnvelope, StageActorReceive},
};

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_started_from_config(config).expect("system")
}

struct NoopReceive;

impl StageActorReceive for NoopReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Ok(())
  }
}

struct RecordingReceive {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl StageActorReceive for RecordingReceive {
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    if let Some(value) = envelope.message().downcast_ref::<u32>() {
      self.values.lock().push(*value);
    }
    Ok(())
  }
}

struct RecordingSystemMessageSender {
  messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
}

impl RecordingSystemMessageSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, Self) {
    let messages = ArcShared::new(SpinSyncMutex::new(Vec::<SystemMessage>::new()));
    let sender = Self { messages: messages.clone() };
    (messages, sender)
  }
}

impl ActorRefSender for RecordingSystemMessageSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let system_message = message.downcast_ref::<SystemMessage>().expect("system message payload");
    self.messages.lock().push(system_message.clone());
    Ok(SendOutcome::Delivered)
  }
}

fn temp_actor_with_recording_sender(
  system: &ActorSystem,
  pid: Pid,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>) {
  let (messages, sender) = RecordingSystemMessageSender::new();
  let actor = ActorRef::new(pid, ActorRefSenderShared::new(Box::new(sender)));
  let _name = system.state().register_temp_actor(actor.clone());
  (actor, messages)
}

#[test]
fn actor_ref_accepts_messages_after_drain() {
  let system = build_system();
  let values = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let stage_actor = StageActor::new(&system, Box::new(RecordingReceive { values: values.clone() }));
  let mut actor_ref = stage_actor.actor_ref().clone();

  actor_ref.try_tell(AnyMessage::new(42_u32)).expect("enqueue");
  stage_actor.drain_pending().expect("drain");

  assert_eq!(*values.lock(), vec![42_u32]);
}

#[test]
fn become_replaces_receive_callback() {
  let system = build_system();
  let first = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let second = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let stage_actor = StageActor::new(&system, Box::new(RecordingReceive { values: first.clone() }));
  let mut actor_ref = stage_actor.actor_ref().clone();

  actor_ref.try_tell(AnyMessage::new(1_u32)).expect("first enqueue");
  stage_actor.drain_pending().expect("first drain");
  stage_actor.r#become(Box::new(RecordingReceive { values: second.clone() }));
  actor_ref.try_tell(AnyMessage::new(2_u32)).expect("second enqueue");
  stage_actor.drain_pending().expect("second drain");

  assert_eq!(*first.lock(), vec![1_u32]);
  assert_eq!(*second.lock(), vec![2_u32]);
}

#[test]
fn watch_and_unwatch_deliver_system_messages_from_stage_actor_pid() {
  let system = build_system();
  let target_pid = system.allocate_pid();
  let (target, messages) = temp_actor_with_recording_sender(&system, target_pid);
  let stage_actor = StageActor::new(&system, Box::new(NoopReceive));
  let stage_actor_pid = stage_actor.actor_ref().pid();

  stage_actor.watch(&target).expect("watch");
  stage_actor.unwatch(&target).expect("unwatch");

  assert_eq!(*messages.lock(), vec![SystemMessage::Watch(stage_actor_pid), SystemMessage::Unwatch(stage_actor_pid)]);
}

#[test]
fn stop_notifies_watchers() {
  let system = build_system();
  let watcher_pid = system.allocate_pid();
  let (_watcher, messages) = temp_actor_with_recording_sender(&system, watcher_pid);
  let stage_actor = StageActor::new(&system, Box::new(NoopReceive));

  system
    .state()
    .send_system_message(stage_actor.actor_ref().pid(), SystemMessage::Watch(watcher_pid))
    .expect("register watcher");
  stage_actor.stop().expect("stop");

  assert_eq!(*messages.lock(), vec![SystemMessage::DeathWatchNotification(stage_actor.actor_ref().pid())]);
}
