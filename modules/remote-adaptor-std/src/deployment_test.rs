use std::{
  string::{String, ToString},
  time::Duration,
};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_path::ActorPathParser,
    actor_ref_provider::LocalActorRefProviderInstaller,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{DeployableFactoryError, Props},
  },
  event::stream::{AddressTerminatedEvent, EventStreamEvent},
  serialization::{builtin::STRING_ID, default_serialization_extension_id},
  system::{ActorSystem, remote::RemotingConfig},
};
use fraktor_remote_core_rs::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::OutboundEnvelope,
  extension::{
    EventPublisher, Remote, RemoteDeploymentOutcome, RemoteDeploymentResponse, RemoteEvent, RemoteShared, Remoting,
  },
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  wire::{
    AckPdu, ControlPdu, HandshakePdu, RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest,
    RemoteDeploymentCreateSuccess, RemoteDeploymentFailureCode, RemoteDeploymentPdu, WireFrame,
  },
};

use super::{DeploymentResponseDispatcher, handle_create_request, subscribe_address_terminated};

struct TestActor;

struct NoopRemoteTransport {
  addresses: Vec<Address>,
}

impl NoopRemoteTransport {
  fn new(addresses: Vec<Address>) -> Self {
    Self { addresses }
  }
}

impl RemoteTransport for NoopRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn connect_peer(&mut self, _remote: &Address) -> Result<(), TransportError> {
    Ok(())
  }

  fn send(&mut self, _envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    Ok(())
  }

  fn send_control(&mut self, _remote: &Address, _pdu: ControlPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_flush_request(&mut self, _remote: &Address, _pdu: ControlPdu, _lane_id: u32) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_ack(&mut self, _remote: &Address, _pdu: AckPdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn send_handshake(&mut self, _remote: &Address, _pdu: HandshakePdu) -> Result<(), TransportError> {
    Ok(())
  }

  fn schedule_handshake_timeout(
    &mut self,
    _authority: &TransportEndpoint,
    _timeout: Duration,
    _generation: u64,
  ) -> Result<(), TransportError> {
    Ok(())
  }

  fn addresses(&self) -> &[Address] {
    &self.addresses
  }

  fn default_address(&self) -> Option<&Address> {
    self.addresses.first()
  }

  fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
    self.default_address()
  }

  fn quarantine(
    &mut self,
    _address: &Address,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    Ok(())
  }
}

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn system_with_factory() -> ActorSystem {
  let config = std_actor_system_config(TestTickDriver::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
    .with_remoting_config(RemotingConfig::default().with_canonical_port(2552));
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should start");
  system.extended().register_deployable_actor_factory("echo", |payload: AnyMessage| {
    if payload.downcast_ref::<String>().map(String::as_str) != Some("payload") {
      return Err(DeployableFactoryError::new("unexpected payload"));
    }
    Ok(Props::from_fn(|| TestActor))
  });
  system
}

fn remote_shared_for_system(system: &ActorSystem) -> RemoteShared {
  RemoteShared::new(Remote::new(
    NoopRemoteTransport::new(vec![Address::new("local-sys", "127.0.0.1", 2551)]),
    RemoteConfig::new("127.0.0.1").with_allowed_remote_host("10.0.0.1"),
    EventPublisher::new(system.downgrade()),
    system.extended().register_extension(&default_serialization_extension_id()),
  ))
}

fn string_payload(value: &str) -> Bytes {
  let bytes = value.as_bytes();
  let mut payload = Vec::with_capacity(4 + bytes.len());
  payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
  payload.extend_from_slice(bytes);
  Bytes::from(payload)
}

fn request(system: &ActorSystem, factory_id: &str, child_name: &str, payload: Bytes) -> RemoteDeploymentCreateRequest {
  let parent = system.user_guardian_ref().path().expect("user guardian path").to_canonical_uri();
  RemoteDeploymentCreateRequest::new(
    1,
    2,
    parent,
    child_name.to_string(),
    factory_id.to_string(),
    String::from("origin@127.0.0.1:2551"),
    STRING_ID.value(),
    None,
    payload,
  )
}

#[test]
fn create_request_creates_actor_and_returns_canonical_path() {
  let system = system_with_factory();
  let serialization = system.extended().register_extension(&default_serialization_extension_id());

  let pdu =
    handle_create_request(&system, &serialization, request(&system, "echo", "worker", string_payload("payload")));

  let RemoteDeploymentPdu::CreateSuccess(success) = pdu else {
    panic!("valid request should succeed: {pdu:?}");
  };
  assert_eq!(success.correlation_hi(), 1);
  assert_eq!(success.correlation_lo(), 2);
  assert!(success.actor_path().ends_with("/user/worker"));
  assert!(success.actor_path().starts_with("fraktor.tcp://default-system@localhost:2552/"));
}

#[test]
fn duplicate_child_name_returns_structured_failure() {
  let system = system_with_factory();
  let serialization = system.extended().register_extension(&default_serialization_extension_id());
  let request = request(&system, "echo", "dup-worker", string_payload("payload"));
  let parent_path = ActorPathParser::parse(request.target_parent_path()).expect("parent path should parse");
  let parent = system
    .pid_by_path(&parent_path)
    .unwrap_or_else(|| system.resolve_actor_ref(parent_path).expect("parent path should resolve").pid());
  system.state().assign_name(Some(parent), Some(request.child_name()), Pid::new(9_999, 0)).expect("reserve child name");

  let pdu = handle_create_request(&system, &serialization, request);

  let RemoteDeploymentPdu::CreateFailure(failure) = pdu else {
    panic!("duplicate name should fail: {pdu:?}");
  };
  assert_eq!(failure.code(), RemoteDeploymentFailureCode::DuplicateChildName);
}

#[test]
fn unknown_factory_id_returns_structured_failure() {
  let system = system_with_factory();
  let serialization = system.extended().register_extension(&default_serialization_extension_id());

  let pdu =
    handle_create_request(&system, &serialization, request(&system, "missing", "worker", string_payload("payload")));

  let RemoteDeploymentPdu::CreateFailure(failure) = pdu else {
    panic!("unknown factory should fail");
  };
  assert_eq!(failure.code(), RemoteDeploymentFailureCode::UnknownFactoryId);
}

#[test]
fn invalid_payload_returns_deserialization_failure() {
  let system = system_with_factory();
  let serialization = system.extended().register_extension(&default_serialization_extension_id());

  let pdu =
    handle_create_request(&system, &serialization, request(&system, "echo", "worker", Bytes::from_static(b"bad")));

  let RemoteDeploymentPdu::CreateFailure(failure) = pdu else {
    panic!("invalid payload should fail");
  };
  assert_eq!(failure.code(), RemoteDeploymentFailureCode::DeserializationFailed);
}

#[test]
fn unexpected_payload_returns_factory_rejected_failure() {
  let system = system_with_factory();
  let serialization = system.extended().register_extension(&default_serialization_extension_id());

  let pdu =
    handle_create_request(&system, &serialization, request(&system, "echo", "worker", string_payload("unexpected")));

  assert_eq!(
    pdu,
    RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
      1,
      2,
      RemoteDeploymentFailureCode::SpawnFailed,
      String::from("unexpected payload"),
    ))
  );
}

#[test]
fn deployment_response_dispatcher_tolerates_dropped_receiver() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let receiver = dispatcher.register_remote_request(&remote, 11, 12, target, 10);
  drop(receiver);

  dispatcher.complete(RemoteDeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
    11,
    12,
    RemoteDeploymentFailureCode::SpawnFailed,
    String::from("late"),
  )));
}

#[test]
fn deployment_response_dispatcher_completes_registered_success() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let receiver = dispatcher.register_remote_request(&remote, 7, 8, target, 10);

  dispatcher.complete(RemoteDeploymentResponse::Success(RemoteDeploymentCreateSuccess::new(
    7,
    8,
    String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/created"),
  )));

  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should complete");
  assert!(matches!(response, RemoteDeploymentResponse::Success(_)));
}

#[test]
fn deployment_response_dispatcher_cancel_disconnects_receiver() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let receiver = dispatcher.register_remote_request(&remote, 13, 14, target, 10);

  dispatcher.cancel_remote_request(&remote, 13, 14);

  assert!(receiver.recv_timeout(Duration::from_millis(10)).is_err());
}

#[test]
fn address_terminated_subscription_fails_matching_pending_deployment() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let _subscription = subscribe_address_terminated(&system, remote.clone(), dispatcher.clone());
  let receiver = dispatcher.register_remote_request(&remote, 1, 2, target, 10);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "Deemed unreachable by remote failure detector",
    20,
  )));

  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should fail");
  assert_eq!(
    response,
    RemoteDeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
      1,
      2,
      RemoteDeploymentFailureCode::AddressTerminated,
      String::from(
        "remote deployment target address terminated: authority=remote-sys@10.0.0.1:2552, reason=Deemed unreachable by remote failure detector",
      ),
    ))
  );
}

#[test]
fn address_terminated_subscription_preserves_pending_create_request_outcomes() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  remote.start().expect("remote should start");
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let _subscription = subscribe_address_terminated(&system, remote.clone(), dispatcher.clone());
  let receiver = dispatcher.register_remote_request(&remote, 1, 2, target.clone(), 10);
  let request = RemoteDeploymentCreateRequest::new(
    9,
    10,
    String::from("fraktor.tcp://local-sys@127.0.0.1:2551/user"),
    String::from("worker"),
    String::from("echo"),
    target.to_string(),
    STRING_ID.value(),
    None,
    Bytes::from_static(b"payload"),
  );
  remote
    .handle_event(RemoteEvent::InboundFrameReceived {
      authority: TransportEndpoint::new(target.to_string()),
      frame:     WireFrame::Deployment(RemoteDeploymentPdu::CreateRequest(request)),
      now_ms:    15,
    })
    .expect("create request should produce an outcome");

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "Deemed unreachable by remote failure detector",
    20,
  )));

  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should fail");
  assert!(matches!(
    response,
    RemoteDeploymentResponse::Failure(failure)
      if failure.correlation_hi() == 1
        && failure.correlation_lo() == 2
        && failure.code() == RemoteDeploymentFailureCode::AddressTerminated
  ));
  let outcomes = remote.drain_deployment_outcomes();
  assert!(matches!(
    outcomes.as_slice(),
    [RemoteDeploymentOutcome::CreateRequested { request, .. }]
      if request.correlation_hi() == 9 && request.correlation_lo() == 10
  ));
}

#[test]
fn replayed_old_address_termination_is_ignored_for_new_pending_deployment() {
  let system = system_with_factory();
  let remote = remote_shared_for_system(&system);
  let dispatcher = DeploymentResponseDispatcher::default();
  let target = Address::new("remote-sys", "10.0.0.1", 2552);
  let _subscription = subscribe_address_terminated(&system, remote.clone(), dispatcher.clone());
  let receiver = dispatcher.register_remote_request(&remote, 3, 4, target, 50);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "old replayed termination",
    20,
  )));

  assert!(receiver.try_recv().is_err());
  dispatcher.cancel_remote_request(&remote, 3, 4);
}
