use core::any::TypeId;

use fraktor_actor_core_rs::core::kernel::actor::{
  Pid,
  actor_ref::{ActorRef, ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessage,
};

use crate::receptionist::{Registered, ServiceKey};

struct StubSender;
impl ActorRefSender for StubSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

fn stub_actor_ref(id: u64) -> ActorRef {
  crate::test_support::actor_ref_with_sender(Pid::new(id, 0), StubSender)
}

// --- new / getter tests ---

#[test]
fn new_should_store_fields() {
  let actor_ref = stub_actor_ref(1);
  let reg = Registered::new("svc", TypeId::of::<u32>(), actor_ref);

  assert_eq!(reg.service_id(), "svc");
  assert_eq!(reg.type_id(), TypeId::of::<u32>());
}

// --- is_for_key tests ---

#[test]
fn is_for_key_returns_true_for_matching_key() {
  let key = ServiceKey::<u32>::new("svc");
  let reg = Registered::new("svc", TypeId::of::<u32>(), stub_actor_ref(1));

  assert!(reg.is_for_key(&key));
}

#[test]
fn is_for_key_returns_false_for_different_service_id() {
  let key = ServiceKey::<u32>::new("other");
  let reg = Registered::new("svc", TypeId::of::<u32>(), stub_actor_ref(1));

  assert!(!reg.is_for_key(&key));
}

#[test]
fn is_for_key_returns_false_for_different_type_id() {
  let key = ServiceKey::<u64>::new("svc");
  let reg = Registered::new("svc", TypeId::of::<u32>(), stub_actor_ref(1));

  assert!(!reg.is_for_key(&key));
}

// --- service_instance tests ---

#[test]
fn service_instance_returns_typed_ref_for_matching_key() {
  let key = ServiceKey::<u32>::new("svc");
  let actor_ref = stub_actor_ref(42);
  let reg = Registered::new("svc", TypeId::of::<u32>(), actor_ref);

  let typed_ref = reg.service_instance(&key).expect("should succeed for matching key");
  assert_eq!(typed_ref.pid(), Pid::new(42, 0));
}

#[test]
fn service_instance_returns_error_for_mismatched_service_id() {
  let key = ServiceKey::<u32>::new("other");
  let reg = Registered::new("svc", TypeId::of::<u32>(), stub_actor_ref(1));

  let result = reg.service_instance(&key);
  assert!(result.is_err(), "should fail when service_id does not match");
}

#[test]
fn service_instance_returns_error_for_mismatched_type_id() {
  let key = ServiceKey::<u64>::new("svc");
  let reg = Registered::new("svc", TypeId::of::<u32>(), stub_actor_ref(1));

  let result = reg.service_instance(&key);
  assert!(result.is_err(), "should fail when type_id does not match");
}
