use core::any::TypeId;

use fraktor_utils_core_rs::core::sync::SharedAccess;

use crate::core::kernel::{
  actor::{
    Actor, ActorContext, Address, Pid,
    actor_path::{ActorPathParser, ActorPathScheme},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::tests::TestTickDriver,
    setup::ActorSystemConfig,
  },
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct TempProbeSender;

impl ActorRefSender for TempProbeSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}

#[test]
fn local_actor_ref_provider_exposes_guardians_dead_letters_and_temp_path() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());

  assert!(provider.root_guardian().is_some());
  assert!(provider.guardian().is_some());
  assert!(provider.system_guardian().is_some());
  assert_eq!(provider.temp_path().to_relative_string(), "/user/temp");

  let mut dead_letters = provider.dead_letters();
  dead_letters.tell(AnyMessage::new(String::from("probe")));
  assert!(!system.dead_letters().is_empty());
}

#[test]
fn local_actor_ref_provider_unbound_defaults_are_safe() {
  let provider = LocalActorRefProvider::default();

  assert_eq!(provider.root_path().to_canonical_uri(), "fraktor://cellactor/user");
  assert_eq!(provider.temp_path().to_relative_string(), "/user/temp");
  assert!(provider.root_guardian().is_none());
  assert!(provider.guardian().is_none());
  assert!(provider.system_guardian().is_none());
  assert!(provider.root_guardian_at(&Address::local("cellactor")).is_none());
  assert!(provider.deployer().is_none());
  assert!(provider.temp_container().is_none());
  assert!(provider.register_temp_actor(ActorRef::null()).is_none());
  assert!(provider.temp_actor("missing").is_none());
  assert!(provider.get_external_address_for(&Address::local("cellactor")).is_none());
  assert!(provider.get_default_address().is_none());
  assert!(provider.termination_signal().is_terminated());

  let error = provider.temp_path_with_prefix("reply").expect_err("unbound temp path");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "LocalActorRefProvider.state not initialized")]
fn local_actor_ref_provider_unbound_dead_letters_debug_asserts() {
  let provider = LocalActorRefProvider::default();

  let _dead_letters = provider.dead_letters();
}

#[test]
fn local_actor_ref_provider_exposes_root_path_and_resolves_actor_ref_str() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-compat");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let child = system.actor_of_named(&Props::from_fn(|| ProbeActor), "provider-child").expect("child");
  let canonical = child.actor_ref().canonical_path().expect("canonical path").to_canonical_uri();

  let mut provider = LocalActorRefProvider::new_with_state(&system.state());

  assert_eq!(provider.root_path().to_canonical_uri(), "fraktor://provider-compat/user");
  let resolved = provider.resolve_actor_ref_str(&canonical).expect("resolve actor ref");
  assert_eq!(resolved, child.actor_ref().clone());
}

#[test]
fn local_actor_ref_provider_resolves_guardians_and_reports_missing_local_path() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-guards");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let mut provider = LocalActorRefProvider::new_with_state(&system.state());

  let user_path = ActorPathParser::parse("fraktor://provider-guards/user").expect("user path");
  let system_path = ActorPathParser::parse("fraktor://provider-guards/system").expect("system path");

  assert_eq!(provider.resolve_actor_ref(user_path).expect("user guardian"), provider.guardian().expect("guardian"));
  assert_eq!(
    provider.resolve_actor_ref(system_path).expect("system guardian"),
    provider.system_guardian().expect("system guardian")
  );

  let missing_path = ActorPathParser::parse("fraktor://provider-guards/user/missing").expect("missing path");
  let error = provider.resolve_actor_ref(missing_path).expect_err("missing path");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_temp_helpers_handle_empty_prefix_and_invalid_path() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-temp");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());

  // 空 prefix は LocalActorRefProvider 側で "tmp" prefix にフォールバックする契約。
  let generated = provider.temp_path_with_prefix("").expect("empty prefix");
  assert!(generated.to_relative_string().starts_with("/user/temp/tmp-"));

  let invalid = provider.root_path().child("not-temp").child("name");
  let error = provider.unregister_temp_actor_path(&invalid).expect_err("invalid temp path");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_external_address_rejects_unrelated_local_system() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-address");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());

  assert!(provider.root_guardian_at(&Address::local("other-system")).is_none());
  assert!(provider.get_external_address_for(&Address::local("other-system")).is_none());
}

#[test]
fn local_actor_ref_provider_supports_temp_actor_round_trip() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());
  let temp_ref = ActorRef::new_with_builtin_lock(Pid::new(4242, 0), TempProbeSender);

  let name = provider.register_temp_actor(temp_ref.clone()).expect("temp actor name");
  let path = provider.temp_path().child(&name);

  assert_eq!(provider.temp_actor(&name), Some(temp_ref.clone()));

  let mut resolver = LocalActorRefProvider::new_with_state(&system.state());
  let resolved = resolver.resolve_actor_ref(path).expect("resolve temp actor");
  assert_eq!(resolved, temp_ref);

  provider.unregister_temp_actor(&name);
  assert!(provider.temp_actor(&name).is_none());
}

#[test]
fn local_actor_ref_provider_exposes_classic_contract_helpers() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-helpers");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());

  let root_at = provider.root_guardian_at(&Address::local("provider-helpers")).expect("root guardian at local");
  assert_eq!(root_at.pid(), provider.root_guardian().expect("root guardian").pid());
  assert!(provider.root_guardian_at(&Address::remote("other", "127.0.0.1", 2552)).is_none());

  assert!(provider.deployer().is_some());
  assert_eq!(provider.get_default_address(), Some(Address::local("provider-helpers")));
  assert_eq!(
    provider.get_external_address_for(&Address::local("provider-helpers")),
    Some(Address::local("provider-helpers"))
  );

  let signal = provider.termination_signal();
  assert!(!signal.is_terminated());
  system.state().mark_terminated();
  assert!(signal.is_terminated());
}

#[test]
fn local_actor_ref_provider_resolve_actor_ref_str_rejects_invalid_path() {
  let state = SystemStateShared::new(SystemState::new());
  let mut provider = LocalActorRefProvider::new_with_state(&state);

  let error = provider.resolve_actor_ref_str("not a canonical actor path").expect_err("invalid path must fail");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_rejects_remote_path_resolution() {
  let state = SystemStateShared::new(SystemState::new());
  let mut provider = LocalActorRefProvider::new_with_state(&state);
  let path = ActorPathParser::parse("fraktor://sys@node:2552/user/worker").expect("remote path");

  let error = provider.resolve_actor_ref(path).expect_err("remote path must fail");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_does_not_keep_system_state_alive_after_registration() {
  let state = SystemStateShared::new(SystemState::new());
  let weak = state.downgrade();

  {
    let actor_ref_provider_handle_shared =
      ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&state));
    state.install_actor_ref_provider(&actor_ref_provider_handle_shared).expect("install provider");
  }

  drop(state);

  assert!(weak.upgrade().is_none(), "provider registration must not create a strong reference cycle");
}

#[test]
fn actor_ref_provider_shared_resolves_actor_refs_via_shared_borrow() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-shared");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let child = system.actor_of_named(&Props::from_fn(|| ProbeActor), "provider-child").expect("child");
  let canonical = child.actor_ref().canonical_path().expect("canonical path").to_canonical_uri();

  let actor_ref_provider_handle_shared =
    ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&system.state()));

  let resolved = actor_ref_provider_handle_shared.resolve_actor_ref_str(&canonical).expect("resolve actor ref");
  assert_eq!(resolved, child.actor_ref().clone());
}

#[test]
fn actor_ref_provider_shared_delegates_shared_access() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-shared-full");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let child = system.actor_of_named(&Props::from_fn(|| ProbeActor), "provider-child").expect("child");
  let canonical = child.actor_ref().canonical_path().expect("canonical path").to_canonical_uri();
  let shared = ActorRefProviderHandleShared::new(LocalActorRefProvider::new_with_state(&system.state()));

  assert_eq!(shared.inner_type_id(), TypeId::of::<LocalActorRefProvider>());
  shared.with_read(|handle| {
    assert_eq!(handle.supported_schemes(), &[ActorPathScheme::Fraktor]);
  });
  let resolved = shared.with_write(|handle| handle.resolve_actor_ref_str(&canonical).expect("resolve via write"));
  assert_eq!(resolved, child.actor_ref().clone());
}

#[test]
fn local_actor_ref_provider_accessors_and_resolve_cover_public_contract() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-accessors");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let child = system.actor_of_named(&Props::from_fn(|| ProbeActor), "provider-child").expect("child");
  let canonical = child.actor_ref().canonical_path().expect("canonical path").to_canonical_uri();
  let child_path = ActorPathParser::parse(&canonical).expect("canonical child path");
  let mut provider = LocalActorRefProvider::new_with_state(&system.state());

  assert_eq!(provider.supported_schemes(), &[ActorPathScheme::Fraktor]);
  assert_eq!(provider.actor_ref(child_path.clone()).expect("actor ref"), child.actor_ref().clone());
  assert_eq!(provider.resolve_actor_ref(child_path.clone()).expect("resolve path"), child.actor_ref().clone());
  assert_eq!(provider.resolve_actor_ref_str(&canonical).expect("resolve str"), child.actor_ref().clone());
}

#[test]
fn local_actor_ref_provider_temp_actor_path_round_trip() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-temp-path");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());
  let prefixed = provider.temp_path_with_prefix("reply").expect("prefixed temp path");
  assert!(prefixed.to_relative_string().starts_with("/user/temp/reply-"));
  let temp_container = provider.temp_container().expect("temp container");
  assert_eq!(temp_container.path().expect("temp path").to_relative_string(), "/user/temp");

  let temp_ref = ActorRef::new_with_builtin_lock(Pid::new(6262, 0), TempProbeSender);
  let name = provider.register_temp_actor(temp_ref.clone()).expect("temp actor");
  assert_eq!(provider.temp_actor(&name), Some(temp_ref.clone()));
  let path = provider.temp_path().child(&name);
  provider.unregister_temp_actor_path(&path).expect("unregister by path");
  assert!(provider.temp_actor(&name).is_none());
}

#[test]
fn local_actor_ref_provider_temp_actor_name_round_trip() {
  let props = Props::from_fn(|| ProbeActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("provider-temp-name");
  let system = ActorSystem::create_with_config(&props, config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());
  let temp_ref = ActorRef::new_with_builtin_lock(Pid::new(6363, 0), TempProbeSender);
  let name = provider.register_temp_actor(temp_ref).expect("temp actor");
  provider.unregister_temp_actor(&name);
  assert!(provider.temp_actor(&name).is_none());
}
