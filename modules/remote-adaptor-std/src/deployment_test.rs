use std::string::{String, ToString};

use bytes::Bytes;
use fraktor_actor_adaptor_std_rs::{system::std_actor_system_config, tick_driver::TestTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    actor_ref_provider::LocalActorRefProviderInstaller,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{DeployableFactoryError, Props},
  },
  serialization::{builtin::STRING_ID, default_serialization_extension_id},
  system::{ActorSystem, remote::RemotingConfig},
};
use fraktor_remote_core_rs::wire::{RemoteDeploymentCreateRequest, RemoteDeploymentFailureCode, RemoteDeploymentPdu};

use super::handle_create_request;

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

  let _ =
    handle_create_request(&system, &serialization, request(&system, "echo", "dup-worker", string_payload("payload")));
  let pdu =
    handle_create_request(&system, &serialization, request(&system, "echo", "dup-worker", string_payload("payload")));

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
