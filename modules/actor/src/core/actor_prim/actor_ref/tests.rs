use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::{
    Actor, ActorCell, ActorContextGeneric, Pid,
    actor_path::ActorPathScheme,
    actor_ref::{ActorRef, ActorRefSender},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::{ActorSystemConfig, RemotingConfig, SystemState},
};

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(&self, _message: AnyMessage) -> Result<(), SendError<NoStdToolbox>> {
    Ok(())
  }
}

#[test]
fn tell_delegates_to_sender() {
  let sender = ArcShared::new(TestSender);
  let pid = Pid::new(5, 1);
  let reference: ActorRef = ActorRef::new(pid, sender);
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

fn build_actor_ref_with_system(remoting: Option<RemotingConfig>) -> ActorRef {
  let state = ArcShared::new(SystemState::new());
  let mut config = ActorSystemConfig::default().with_system_name("canonical-test");
  if let Some(remoting_config) = remoting {
    config = config.with_remoting_config(remoting_config);
  }
  state.apply_actor_system_config(&config);

  let props = Props::from_fn(|| NoopActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root cell");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("child cell");
  state.register_cell(child.clone());

  child.actor_ref()
}

#[test]
fn canonical_path_uses_canonical_authority_when_available() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let reference = build_actor_ref_with_system(Some(remoting));

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(canonical.parts().authority_endpoint(), Some("example.com:2552".to_string()));
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_returns_local_when_remoting_disabled() {
  let reference = build_actor_ref_with_system(None);

  let canonical = reference.canonical_path().expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::Fraktor);
  assert_eq!(canonical.parts().authority_endpoint(), None);
  assert_eq!(canonical.to_relative_string(), "/user/worker");

  let local = reference.path().expect("local path");
  assert_eq!(local.parts().authority_endpoint(), None);
}

#[test]
fn canonical_path_is_none_without_system_state() {
  let sender = ArcShared::new(TestSender);
  let reference: ActorRef = ActorRef::new(Pid::new(1, 0), sender);
  assert!(reference.canonical_path().is_none());
}
