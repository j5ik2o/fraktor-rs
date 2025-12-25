use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  actor::{
    Actor, ActorCell, ActorContextGeneric, Pid,
    actor_path::ActorPathScheme,
    actor_ref::{ActorRef, ActorRefSender},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  scheduler::{ManualTestDriver, SchedulerConfig, TickDriverConfig},
  system::{ActorSystemConfig, RemotingConfig, SystemState, SystemStateShared},
};

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(
    &mut self,
    _message: AnyMessage,
  ) -> Result<crate::core::actor::actor_ref::SendOutcome, SendError<NoStdToolbox>> {
    Ok(crate::core::actor::actor_ref::SendOutcome::Delivered)
  }
}

#[test]
fn tell_delegates_to_sender() {
  let pid = Pid::new(5, 1);
  let reference: ActorRef = ActorRef::new(pid, TestSender);
  assert!(reference.tell(AnyMessage::new("ping")).is_ok());
}

#[test]
fn null_sender_returns_error() {
  let reference: ActorRef = ActorRef::null();
  let error = reference.tell(AnyMessage::new("ping")).unwrap_err();
  assert!(matches!(error, SendError::Closed(_)));
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

/// Builds an ActorRef with an associated SystemState.
///
/// Returns both the ActorRef and the SystemStateShared to keep the system state alive.
/// Since ActorRef now uses weak references to SystemState, the returned SystemStateShared
/// must be kept alive for the ActorRef's path methods to work.
fn build_actor_ref_with_system(remoting: Option<RemotingConfig>) -> (ActorRef, SystemStateShared) {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::<NoStdToolbox>::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let mut config = ActorSystemConfig::default()
    .with_system_name("canonical-test")
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver);
  if let Some(remoting_config) = remoting {
    config = config.with_remoting_config(remoting_config);
  }
  let state = SystemStateShared::new(SystemState::build_from_config(&config).expect("state"));

  let props = Props::from_fn(|| NoopActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root cell");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("child cell");
  state.register_cell(child.clone());

  (child.actor_ref(), state)
}

#[test]
fn canonical_path_uses_canonical_authority_when_available() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let (reference, _state) = build_actor_ref_with_system(Some(remoting));

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(canonical.parts().authority_endpoint(), Some("example.com:2552".to_string()));
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_returns_local_when_remoting_disabled() {
  let (reference, _state) = build_actor_ref_with_system(None);

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::Fraktor);
  assert_eq!(canonical.parts().authority_endpoint(), None);
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_is_none_without_system_state() {
  let reference: ActorRef = ActorRef::new(Pid::new(1, 0), TestSender);
  assert!(reference.canonical_path().is_none());
}
