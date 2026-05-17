use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::SendError,
    messaging::{AnyMessage, system_message::SystemMessage},
    scheduler::SchedulerConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{ActorBackedSourceRefLogic, SourceRef};
use crate::{
  dsl::Source,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  materialization::StreamNotUsed,
  source_logic::SourceLogic,
};

struct RecordingSender {
  system_messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
  user_messages:   ArcShared<SpinSyncMutex<usize>>,
}

impl RecordingSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>, Self) {
    let system_messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
    let user_messages = ArcShared::new(SpinSyncMutex::new(0_usize));
    let sender = Self { system_messages: system_messages.clone(), user_messages: user_messages.clone() };
    (system_messages, user_messages, sender)
  }
}

impl ActorRefSender for RecordingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(system_message) = message.downcast_ref::<SystemMessage>() {
      self.system_messages.lock().push(system_message.clone());
    } else {
      *self.user_messages.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| config.with_scheduler_config(scheduler))
}

fn temp_recording_actor(
  system: &ActorSystem,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>) {
  let pid = system.allocate_pid();
  temp_recording_actor_with_pid(system, pid)
}

fn temp_recording_actor_with_pid(
  system: &ActorSystem,
  pid: Pid,
) -> (ActorRef, ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, ArcShared<SpinSyncMutex<usize>>) {
  let (system_messages, user_messages, sender) = RecordingSender::new();
  let actor_ref = ActorRef::new(pid, ActorRefSenderShared::new(Box::new(sender)));
  let _name = system.state().register_temp_actor(actor_ref.clone());
  (actor_ref, system_messages, user_messages)
}

#[test]
fn into_source_consumes_source_ref() {
  let handoff = StreamRefHandoff::<u32>::new();
  let source_ref = SourceRef::new(handoff, StreamRefEndpointSlot::new());

  let _source: Source<u32, StreamNotUsed> = source_ref.into_source();
}

#[test]
fn actor_backed_source_ref_watches_target_when_attached() {
  let system = build_system();
  let (target, system_messages, user_messages) = temp_recording_actor(&system);
  let mut logic = ActorBackedSourceRefLogic::<u32>::new(target);

  logic.attach_actor_system(system);

  let endpoint_pid = logic.endpoint_actor_ref().expect("endpoint actor ref").pid();
  assert_eq!(*system_messages.lock(), vec![SystemMessage::Watch(endpoint_pid)]);
  assert_eq!(*user_messages.lock(), 1);
}
