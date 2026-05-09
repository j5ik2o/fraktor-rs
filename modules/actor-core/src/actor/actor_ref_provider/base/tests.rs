use crate::{
  actor::{
    Address,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::ActorRefProvider,
    error::{ActorError, SendError},
    messaging::AnyMessage,
  },
  system::TerminationSignal,
};

#[derive(Default)]
struct StubActorRefProvider {
  last_path: Option<ActorPath>,
}

impl ActorRefProvider for StubActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::Fraktor]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.last_path = Some(path);
    Ok(ActorRef::null())
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

#[test]
fn actor_ref_provider_default_accessors_are_absent_or_local_defaults() {
  let provider = StubActorRefProvider::default();

  assert_eq!(provider.supported_schemes(), &[ActorPathScheme::Fraktor]);
  assert!(provider.root_guardian().is_none());
  assert!(provider.guardian().is_none());
  assert!(provider.system_guardian().is_none());
  assert_eq!(provider.temp_path().to_relative_string(), "/user/temp");
  assert_eq!(provider.root_path().to_canonical_uri(), "fraktor://cellactor/user");
  assert!(provider.root_guardian_at(&Address::local("cellactor")).is_none());
  assert!(provider.deployer().is_none());
  assert!(provider.temp_container().is_none());
  assert!(provider.register_temp_actor(ActorRef::null()).is_none());
  assert!(provider.temp_actor("missing").is_none());
  assert!(provider.get_external_address_for(&Address::local("cellactor")).is_none());
  assert!(provider.get_default_address().is_none());
  assert!(provider.termination_signal().is_terminated());
}

#[test]
fn actor_ref_provider_default_dead_letters_is_closed_null_actor() {
  let provider = StubActorRefProvider::default();
  let mut dead_letters = provider.dead_letters();

  let result = dead_letters.try_tell(AnyMessage::new(String::from("dropped")));

  assert!(matches!(result, Err(SendError::Closed(_))));
}

#[test]
fn actor_ref_provider_default_resolution_delegates_to_actor_ref() {
  let mut provider = StubActorRefProvider::default();
  let path = ActorPath::root().child("worker");

  let mut actor_ref = provider.resolve_actor_ref(path.clone()).expect("resolve");

  assert!(matches!(actor_ref.try_tell(AnyMessage::new("unused")), Err(SendError::Closed(_))));
  assert_eq!(provider.last_path.expect("last path"), path);
}

#[test]
fn actor_ref_provider_default_string_resolution_parses_then_delegates() {
  let mut provider = StubActorRefProvider::default();

  provider.resolve_actor_ref_str("fraktor://cellactor/user/worker").expect("resolve string");

  let path = provider.last_path.expect("last path");
  assert_eq!(path.to_canonical_uri(), "fraktor://cellactor/user/worker");
}

#[test]
fn actor_ref_provider_default_string_resolution_rejects_invalid_path() {
  let mut provider = StubActorRefProvider::default();

  let error = provider.resolve_actor_ref_str("not an actor path").expect_err("invalid path");

  assert!(matches!(error, ActorError::Fatal(_)));
  assert!(provider.last_path.is_none());
}

#[test]
fn actor_ref_provider_default_temp_mutations_are_unsupported() {
  let provider = StubActorRefProvider::default();

  let prefix_error = provider.temp_path_with_prefix("reply").expect_err("prefix unsupported");
  assert!(matches!(prefix_error, ActorError::Fatal(_)));

  provider.unregister_temp_actor("missing");
  let path_error =
    provider.unregister_temp_actor_path(&ActorPath::root().child("temp").child("reply")).expect_err("path");
  assert!(matches!(path_error, ActorError::Fatal(_)));
}
