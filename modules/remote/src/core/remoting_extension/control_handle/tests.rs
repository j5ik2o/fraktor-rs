#![cfg(any(test, feature = "test-support"))]
#![cfg(all(feature = "std", feature = "tokio-transport"))]

use alloc::boxed::Box;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPathParts, GuardianKind},
    actor_ref::ActorRefGeneric,
  },
  error::ActorError,
  event::stream::CorrelationId,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::RemotingControlHandle;
use crate::core::{
  remoting_extension::{RemotingControl, RemotingError, RemotingExtensionConfig},
  transport::{
    LoopbackTransport, RemoteTransport, RemoteTransportShared, TransportBind, TransportChannel, TransportEndpoint,
    TransportError, TransportHandle, inbound::TransportInboundShared,
  },
  watcher::RemoteWatcherCommand,
};

struct NoopActor;

impl Actor<StdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    _message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone, Copy)]
enum TransportFailureMode {
  OpenChannel,
  Send,
}

struct FailingTransport {
  mode: TransportFailureMode,
}

impl FailingTransport {
  fn new(mode: TransportFailureMode) -> Self {
    Self { mode }
  }
}

impl RemoteTransport<StdToolbox> for FailingTransport {
  fn scheme(&self) -> &str {
    "fraktor.test"
  }

  fn spawn_listener(&mut self, _bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    Ok(TransportHandle::new("test-listener"))
  }

  fn open_channel(&mut self, _endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    match self.mode {
      | TransportFailureMode::OpenChannel => Err(TransportError::Io("open channel failure".into())),
      | TransportFailureMode::Send => Ok(TransportChannel::new(1)),
    }
  }

  fn send(
    &mut self,
    _channel: &TransportChannel,
    _payload: &[u8],
    _correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    match self.mode {
      | TransportFailureMode::OpenChannel => Ok(()),
      | TransportFailureMode::Send => Err(TransportError::Io("send heartbeat failure".into())),
    }
  }

  fn close(&mut self, _channel: &TransportChannel) {}

  fn install_backpressure_hook(&mut self, _hook: crate::core::transport::TransportBackpressureHookShared) {}

  fn install_inbound_handler(&mut self, _handler: TransportInboundShared<StdToolbox>) {}
}

fn build_system() -> ActorSystemGeneric<StdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("control-handle-tests");
  let config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()));
  ActorSystemGeneric::new_with_config(&props, &config).expect("actor system")
}

fn build_started_control(mode: TransportFailureMode) -> RemotingControlHandle<StdToolbox> {
  let system = build_system();
  let mut handle = RemotingControlHandle::new(system, RemotingExtensionConfig::default());
  let transport = RemoteTransportShared::new(Box::new(FailingTransport::new(mode)));
  handle.register_remote_transport_shared(transport);
  handle.start().expect("control start");
  handle
}

fn build_started_control_without_transport() -> RemotingControlHandle<StdToolbox> {
  let system = build_system();
  let mut handle = RemotingControlHandle::new(system, RemotingExtensionConfig::default());
  handle.start().expect("control start");
  handle
}

fn build_started_control_with_loopback_transport() -> RemotingControlHandle<StdToolbox> {
  let system = build_system();
  let mut handle = RemotingControlHandle::new(system, RemotingExtensionConfig::default());
  let transport = RemoteTransportShared::new(Box::new(LoopbackTransport::<StdToolbox>::default()));
  handle.register_remote_transport_shared(transport);
  handle.start().expect("control start");
  handle
}

fn remote_address() -> ActorPathParts {
  ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520))).with_guardian(GuardianKind::User)
}

#[test]
fn associate_fails_when_transport_is_not_registered() {
  let mut handle = build_started_control_without_transport();
  let address = remote_address();

  let error = handle.associate(&address).expect_err("associate should fail when transport is not registered");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("remote transport not registered"), "unexpected reason: {reason}");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn associate_propagates_loopback_open_channel_failure() {
  let mut handle = build_started_control_with_loopback_transport();
  let address = remote_address();

  let error = handle.associate(&address).expect_err("associate should fail when loopback endpoint is not bound");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("authority not bound"), "unexpected reason: {reason}");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn associate_propagates_open_channel_failure() {
  let mut handle = build_started_control(TransportFailureMode::OpenChannel);
  let address = remote_address();

  let error = handle.associate(&address).expect_err("associate should fail when open_channel fails");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("open channel failure"), "unexpected reason: {reason}");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn associate_propagates_send_failure() {
  let mut handle = build_started_control(TransportFailureMode::Send);
  let address = remote_address();

  let error = handle.associate(&address).expect_err("associate should fail when send fails");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("send heartbeat failure"), "unexpected reason: {reason}");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn dispatch_remote_watcher_command_returns_error_without_daemon() {
  let handle = build_started_control_without_transport();

  let error = handle
    .dispatch_remote_watcher_command(RemoteWatcherCommand::heartbeat("127.0.0.1:25520"))
    .expect_err("dispatch without daemon should return an error");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("watcher daemon not registered"), "unexpected reason: {reason}");
    }
    | other => panic!("unexpected error variant: {other:?}"),
  }
}

#[test]
fn dispatch_remote_watcher_command_propagates_daemon_send_failure() {
  let handle = build_started_control_without_transport();
  handle.register_remote_watcher_daemon(ActorRefGeneric::null());

  let error = handle
    .dispatch_remote_watcher_command(RemoteWatcherCommand::heartbeat("127.0.0.1:25520"))
    .expect_err("dispatch should fail when watcher daemon mailbox rejects messages");
  match error {
    | RemotingError::TransportUnavailable(reason) => {
      assert!(reason.contains("Closed"), "unexpected reason: {reason}");
    },
    | other => panic!("unexpected error: {other:?}"),
  }
}
