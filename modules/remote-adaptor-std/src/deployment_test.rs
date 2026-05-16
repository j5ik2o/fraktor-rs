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
use fraktor_remote_core_rs::wire::{
  RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentCreateSuccess,
  RemoteDeploymentFailureCode, RemoteDeploymentPdu,
};

use super::{
  DeploymentResponse, DeploymentResponseDispatcher, MAX_STALE_DEPLOYMENT_RESPONSES, handle_create_request,
  subscribe_address_terminated,
};

struct TestActor;

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

impl DeploymentResponseDispatcher {
  fn remote_created_len(&self) -> usize {
    self.state.with_lock(|state| state.remote_created.values().sum())
  }
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
fn deployment_response_dispatcher_bounds_stale_responses() {
  let dispatcher = DeploymentResponseDispatcher::default();

  for index in 0..(MAX_STALE_DEPLOYMENT_RESPONSES + 1) {
    dispatcher.complete(DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
      index as u64,
      0,
      RemoteDeploymentFailureCode::SpawnFailed,
      String::from("late"),
    )));
  }

  assert_eq!(dispatcher.stale_len(), MAX_STALE_DEPLOYMENT_RESPONSES);
}

#[test]
fn deployment_response_dispatcher_records_stale_when_receiver_is_dropped() {
  let dispatcher = DeploymentResponseDispatcher::default();
  let receiver = dispatcher.register(11, 12, "remote-sys@10.0.0.1:2552", 10);
  drop(receiver);

  dispatcher.complete(DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
    11,
    12,
    RemoteDeploymentFailureCode::SpawnFailed,
    String::from("late"),
  )));

  assert_eq!(dispatcher.stale_len(), 1);
}

#[test]
fn successful_deployment_response_tracks_remote_created_child() {
  let dispatcher = DeploymentResponseDispatcher::default();
  let receiver = dispatcher.register(7, 8, "remote-sys@10.0.0.1:2552", 10);

  dispatcher.complete(DeploymentResponse::Success(RemoteDeploymentCreateSuccess::new(
    7,
    8,
    String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/created"),
  )));

  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should complete");
  assert!(matches!(response, DeploymentResponse::Success(_)));
  assert_eq!(dispatcher.remote_created_len(), 1);
}

#[test]
fn address_termination_cleans_remote_created_tracking() {
  let system = system_with_factory();
  let dispatcher = DeploymentResponseDispatcher::default();
  let _subscription = subscribe_address_terminated(&system, dispatcher.clone());
  let receiver = dispatcher.register(9, 10, "remote-sys@10.0.0.1:2552", 10);

  dispatcher.complete(DeploymentResponse::Success(RemoteDeploymentCreateSuccess::new(
    9,
    10,
    String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/created"),
  )));
  let _response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should complete");
  assert_eq!(dispatcher.remote_created_len(), 1);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "Deemed unreachable by remote failure detector",
    20,
  )));

  assert_eq!(dispatcher.remote_created_len(), 0);
}

#[test]
fn address_terminated_subscription_fails_matching_pending_deployment() {
  let system = system_with_factory();
  let dispatcher = DeploymentResponseDispatcher::default();
  let _subscription = subscribe_address_terminated(&system, dispatcher.clone());
  let receiver = dispatcher.register(1, 2, "remote-sys@10.0.0.1:2552", 10);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "Deemed unreachable by remote failure detector",
    20,
  )));

  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should fail");
  assert_eq!(
    response,
    DeploymentResponse::Failure(RemoteDeploymentCreateFailure::new(
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
fn replayed_old_address_termination_is_ignored_for_new_pending_deployment() {
  let system = system_with_factory();
  let dispatcher = DeploymentResponseDispatcher::default();
  let _subscription = subscribe_address_terminated(&system, dispatcher.clone());
  let receiver = dispatcher.register(3, 4, "remote-sys@10.0.0.1:2552", 50);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "old replayed termination",
    20,
  )));

  assert!(receiver.try_recv().is_err());
  dispatcher.cancel(3, 4);
}

#[test]
fn late_deployment_response_after_address_termination_is_stale() {
  let system = system_with_factory();
  let dispatcher = DeploymentResponseDispatcher::default();
  let _subscription = subscribe_address_terminated(&system, dispatcher.clone());
  let receiver = dispatcher.register(5, 6, "remote-sys@10.0.0.1:2552", 10);

  system.event_stream().publish(&EventStreamEvent::AddressTerminated(AddressTerminatedEvent::new(
    "remote-sys@10.0.0.1:2552",
    "Deemed unreachable by remote failure detector",
    20,
  )));
  let response = receiver.recv_timeout(Duration::from_secs(1)).expect("pending deployment should fail");
  assert!(matches!(
    response,
    DeploymentResponse::Failure(failure) if failure.code() == RemoteDeploymentFailureCode::AddressTerminated
  ));

  dispatcher.complete(DeploymentResponse::Success(RemoteDeploymentCreateSuccess::new(
    5,
    6,
    String::from("fraktor.tcp://remote-sys@10.0.0.1:2552/user/late"),
  )));

  assert_eq!(dispatcher.stale_len(), 1);
}
