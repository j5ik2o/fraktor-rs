use crate::core::kernel::{
  actor::{
    Actor, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderShared, LocalActorRefProvider},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::{ActorSystem, state::system_state::SystemState},
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
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default().with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());

  assert!(provider.root_guardian().is_some());
  assert!(provider.guardian().is_some());
  assert!(provider.system_guardian().is_some());
  assert_eq!(provider.temp_path().to_relative_string(), "/user/temp");

  let mut dead_letters = provider.dead_letters();
  dead_letters.tell(crate::core::kernel::actor::messaging::AnyMessage::new(String::from("probe")));
  assert!(!system.dead_letters().is_empty());
}

#[test]
fn local_actor_ref_provider_exposes_root_path_and_resolves_actor_ref_str() {
  let props = Props::from_fn(|| ProbeActor).with_name("user-root");
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default()
    .with_system_name("provider-compat")
    .with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let child = system.actor_of_named(&Props::from_fn(|| ProbeActor), "provider-child").expect("child");
  let canonical = child.actor_ref().canonical_path().expect("canonical path").to_canonical_uri();

  let mut provider = LocalActorRefProvider::new_with_state(&system.state());

  assert_eq!(provider.root_path().to_canonical_uri(), "fraktor://provider-compat/user");
  let resolved = provider.resolve_actor_ref_str(&canonical).expect("resolve actor ref");
  assert_eq!(resolved, child.actor_ref().clone());
}

#[test]
fn local_actor_ref_provider_supports_temp_actor_round_trip() {
  let props = Props::from_fn(|| ProbeActor);
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default().with_tick_driver(tick_driver);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let provider = LocalActorRefProvider::new_with_state(&system.state());
  let temp_ref = ActorRef::new(Pid::new(4242, 0), TempProbeSender);

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
fn local_actor_ref_provider_resolve_actor_ref_str_rejects_invalid_path() {
  let state = crate::core::kernel::system::state::SystemStateShared::new(SystemState::new());
  let mut provider = LocalActorRefProvider::new_with_state(&state);

  let error = provider.resolve_actor_ref_str("not a canonical actor path").expect_err("invalid path must fail");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_rejects_remote_path_resolution() {
  let state = crate::core::kernel::system::state::SystemStateShared::new(SystemState::new());
  let mut provider = LocalActorRefProvider::new_with_state(&state);
  let path = crate::core::kernel::actor::actor_path::ActorPathParser::parse("fraktor://sys@node:2552/user/worker")
    .expect("remote path");

  let error = provider.resolve_actor_ref(path).expect_err("remote path must fail");
  assert!(matches!(error, ActorError::Fatal(_)));
}

#[test]
fn local_actor_ref_provider_does_not_keep_system_state_alive_after_registration() {
  let state = crate::core::kernel::system::state::SystemStateShared::new(SystemState::new());
  let weak = state.downgrade();

  {
    let provider = ActorRefProviderShared::new(LocalActorRefProvider::new_with_state(&state));
    state.install_actor_ref_provider(&provider).expect("install provider");
  }

  drop(state);

  assert!(weak.upgrade().is_none(), "provider registration must not create a strong reference cycle");
}
