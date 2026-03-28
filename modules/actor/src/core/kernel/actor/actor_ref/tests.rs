use alloc::string::ToString;

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_path::ActorPathScheme,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageView},
  props::Props,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{
    ActorSystemConfig,
    remote::RemotingConfig,
    state::{SystemStateShared, system_state::SystemState},
  },
};

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

/// `try_tell` succeeds when the underlying sender accepts the message.
#[test]
fn try_tell_delegates_to_sender() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new(pid, TestSender);
  assert!(reference.try_tell(AnyMessage::new("ping")).is_ok());
}

/// `try_tell` on a null sender reports `Closed`.
#[test]
fn try_tell_on_null_sender_returns_closed() {
  let mut reference: ActorRef = ActorRef::null();
  assert!(matches!(reference.try_tell(AnyMessage::new("ping")), Err(SendError::Closed(_))));
}

/// `try_tell` on a failing sender returns the underlying send error.
#[test]
fn try_tell_on_failing_sender_returns_error() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new(pid, FailingSender);
  assert!(matches!(reference.try_tell(AnyMessage::new("will-fail")), Err(SendError::Closed(_))));
}

/// `try_tell` is a hidden fallible send helper used by infrastructure code such as `ask`.
/// It returns `Result<(), SendError>` so that `ask` can propagate send failures.
#[test]
fn try_tell_returns_result_on_success() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new(pid, TestSender);
  assert!(reference.try_tell(AnyMessage::new("ask-payload")).is_ok());
}

/// `try_tell` propagates the error when the sender fails.
#[test]
fn try_tell_returns_error_on_failure() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new(pid, FailingSender);
  assert!(matches!(reference.try_tell(AnyMessage::new("will-fail")), Err(SendError::Closed(_))));
}

/// `ask` はレスポンスハンドルを返し、結果は future 側で観測する。
#[test]
fn ask_returns_response_handle() {
  let pid = Pid::new(5, 1);
  let mut reference: ActorRef = ActorRef::new(pid, TestSender);
  let _response = reference.ask(AnyMessage::new("ask-payload"));
}

/// `ask` on a failing sender completes the future with `SendFailed`.
#[test]
fn ask_on_failing_sender_completes_future_with_send_failed() {
  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let pid = Pid::new(10, 1);
  let mut reference: ActorRef = ActorRef::new(pid, FailingSender);
  let response = reference.ask(AnyMessage::new("will-fail"));
  assert_eq!(response.sender().pid(), pid);
  let result = response.future().with_write(|future| future.try_take()).expect("future should be ready");
  assert!(matches!(result, Err(crate::core::kernel::messaging::AskError::SendFailed(_))));
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

/// Builds an ActorRef with an associated SystemState.
///
/// Returns both the ActorRef and the SystemStateShared to keep the system state alive.
/// Since ActorRef now uses weak references to SystemState, the returned SystemStateShared
/// must be kept alive for the ActorRef's path methods to work.
fn build_actor_ref_with_system(remoting: Option<RemotingConfig>) -> (ActorRef, SystemStateShared) {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
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
