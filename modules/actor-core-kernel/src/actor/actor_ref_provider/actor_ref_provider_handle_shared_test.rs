use alloc::string::String;
use core::any::TypeId;
use std::sync::{Arc, Mutex};

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::ActorRefProviderHandleShared;
use crate::{
  actor::{
    Address, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, NullSender},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandle},
    deploy::{Deploy, Deployer},
    error::ActorError,
  },
  system::TerminationSignal,
};

const SUPPORTED_SCHEMES: &[ActorPathScheme] = &[ActorPathScheme::Fraktor, ActorPathScheme::FraktorTcp];

#[derive(Clone)]
struct ProviderObservations {
  actor_ref_path:            Arc<Mutex<Option<String>>>,
  resolve_actor_ref_path:    Arc<Mutex<Option<String>>>,
  resolve_actor_ref_str:     Arc<Mutex<Option<String>>>,
  register_temp_actor_calls: Arc<Mutex<u32>>,
  unregister_temp_actor:     Arc<Mutex<Option<String>>>,
  unregister_temp_actor_uri: Arc<Mutex<Option<String>>>,
}

impl ProviderObservations {
  fn new() -> Self {
    Self {
      actor_ref_path:            Arc::new(Mutex::new(None)),
      resolve_actor_ref_path:    Arc::new(Mutex::new(None)),
      resolve_actor_ref_str:     Arc::new(Mutex::new(None)),
      register_temp_actor_calls: Arc::new(Mutex::new(0)),
      unregister_temp_actor:     Arc::new(Mutex::new(None)),
      unregister_temp_actor_uri: Arc::new(Mutex::new(None)),
    }
  }
}

struct StubActorRefProvider {
  observations: ProviderObservations,
}

impl StubActorRefProvider {
  fn new(observations: ProviderObservations) -> Self {
    Self { observations }
  }

  fn actor_ref_with_pid(pid: u64) -> ActorRef {
    ActorRef::with_canonical_path(Pid::new(pid, 0), NullSender, ActorPath::root().child("shared-stub"))
  }
}

impl ActorRefProvider for StubActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    SUPPORTED_SCHEMES
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    *self.observations.actor_ref_path.lock().expect("actor_ref_path") = Some(path.to_canonical_uri());
    Ok(Self::actor_ref_with_pid(20))
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(21))
  }

  fn guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(22))
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(23))
  }

  fn dead_letters(&self) -> ActorRef {
    Self::actor_ref_with_pid(24)
  }

  fn temp_path(&self) -> ActorPath {
    ActorPath::root().child("temp")
  }

  fn root_path(&self) -> ActorPath {
    ActorPath::root()
  }

  fn root_guardian_at(&self, _address: &Address) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(25))
  }

  fn deployer(&self) -> Option<Deployer> {
    let mut deployer = Deployer::new();
    deployer.register("/user/shared", Deploy::new());
    Some(deployer)
  }

  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    *self.observations.resolve_actor_ref_path.lock().expect("resolve_actor_ref_path") = Some(path.to_canonical_uri());
    Ok(Self::actor_ref_with_pid(26))
  }

  fn resolve_actor_ref_str(&mut self, path: &str) -> Result<ActorRef, ActorError> {
    *self.observations.resolve_actor_ref_str.lock().expect("resolve_actor_ref_str") = Some(String::from(path));
    Ok(Self::actor_ref_with_pid(27))
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    Ok(ActorPath::root().child("temp").child(prefix))
  }

  fn temp_container(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(28))
  }

  fn register_temp_actor(&self, _actor: ActorRef) -> Option<String> {
    *self.observations.register_temp_actor_calls.lock().expect("register_temp_actor_calls") += 1;
    Some(String::from("shared-temp"))
  }

  fn unregister_temp_actor(&self, name: &str) {
    *self.observations.unregister_temp_actor.lock().expect("unregister_temp_actor") = Some(String::from(name));
  }

  fn unregister_temp_actor_path(&self, path: &ActorPath) -> Result<(), ActorError> {
    *self.observations.unregister_temp_actor_uri.lock().expect("unregister_temp_actor_uri") =
      Some(path.to_canonical_uri());
    Ok(())
  }

  fn temp_actor(&self, _name: &str) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(29))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }

  fn get_external_address_for(&self, _addr: &Address) -> Option<Address> {
    Some(Address::remote("shared-system", "127.0.0.1", 2553))
  }

  fn get_default_address(&self) -> Option<Address> {
    Some(Address::local("shared-system"))
  }
}

#[test]
fn actor_ref_provider_handle_shared_delegates_and_exposes_shared_access() {
  let observations = ProviderObservations::new();
  let actor_path = ActorPath::root().child("worker");
  let temp_path = ActorPath::root().child("temp").child("shared-temp");
  let shared = ActorRefProviderHandleShared::new(StubActorRefProvider::new(observations.clone()));
  let shared_clone = shared.clone();
  let mut provider = shared_clone.clone();

  assert_eq!(shared.inner_type_id(), TypeId::of::<StubActorRefProvider>());
  assert_eq!(ActorRefProvider::supported_schemes(&provider), SUPPORTED_SCHEMES);
  assert_eq!(shared.get_actor_ref(actor_path.clone()).expect("get_actor_ref").pid(), Pid::new(20, 0));
  assert_eq!(shared.root_guardian().expect("root_guardian").pid(), Pid::new(21, 0));
  assert_eq!(shared.guardian().expect("guardian").pid(), Pid::new(22, 0));
  assert_eq!(shared.system_guardian().expect("system_guardian").pid(), Pid::new(23, 0));
  assert_eq!(shared.dead_letters().pid(), Pid::new(24, 0));
  assert_eq!(shared.temp_path().to_canonical_uri(), "fraktor://cellactor/user/temp");
  assert_eq!(shared.root_path().to_canonical_uri(), "fraktor://cellactor/user");
  assert_eq!(
    shared.root_guardian_at(&Address::local("shared-system")).expect("root_guardian_at").pid(),
    Pid::new(25, 0)
  );
  assert!(shared.deployer().expect("deployer").deploy_for("/user/shared").is_some());
  assert_eq!(shared.resolve_actor_ref(actor_path.clone()).expect("resolve_actor_ref").pid(), Pid::new(26, 0));
  assert_eq!(
    shared.resolve_actor_ref_str("fraktor://cellactor/user/shared-worker").expect("resolve_actor_ref_str").pid(),
    Pid::new(27, 0)
  );
  assert_eq!(provider.actor_ref(actor_path.clone()).expect("actor_ref").pid(), Pid::new(20, 0));
  assert_eq!(provider.resolve_actor_ref(actor_path.clone()).expect("trait resolve_actor_ref").pid(), Pid::new(26, 0));
  assert_eq!(
    provider
      .resolve_actor_ref_str("fraktor://cellactor/user/shared-worker")
      .expect("trait resolve_actor_ref_str")
      .pid(),
    Pid::new(27, 0)
  );
  assert_eq!(
    shared.temp_path_with_prefix("shared-temp").expect("temp_path_with_prefix").to_canonical_uri(),
    temp_path.to_canonical_uri()
  );
  assert_eq!(shared.temp_container().expect("temp_container").pid(), Pid::new(28, 0));
  assert_eq!(shared.register_temp_actor(ActorRef::null()), Some(String::from("shared-temp")));
  shared.unregister_temp_actor("shared-temp");
  assert!(shared.unregister_temp_actor_path(&temp_path).is_ok());
  assert_eq!(shared.temp_actor("shared-temp").expect("temp_actor").pid(), Pid::new(29, 0));
  assert!(shared.termination_signal().is_terminated());
  assert_eq!(
    shared
      .get_external_address_for(&Address::local("shared-system"))
      .expect("get_external_address_for")
      .to_uri_string(),
    "fraktor.tcp://shared-system@127.0.0.1:2553"
  );
  assert_eq!(shared.get_default_address().expect("get_default_address").to_uri_string(), "fraktor://shared-system");

  let supported_scheme_count = shared.with_read(|guard| guard.supported_schemes().len());
  assert_eq!(supported_scheme_count, SUPPORTED_SCHEMES.len());
  let provider_call_pid = shared
    .with_write(|guard| guard.actor_ref(ActorPath::root().child("with-write")).expect("with_write actor_ref").pid());
  assert_eq!(provider_call_pid, Pid::new(20, 0));

  assert_eq!(
    observations.actor_ref_path.lock().expect("actor_ref_path").clone(),
    Some(String::from("fraktor://cellactor/user/with-write"))
  );
  assert_eq!(
    observations.resolve_actor_ref_path.lock().expect("resolve_actor_ref_path").clone(),
    Some(actor_path.to_canonical_uri())
  );
  assert_eq!(
    observations.resolve_actor_ref_str.lock().expect("resolve_actor_ref_str").clone(),
    Some(String::from("fraktor://cellactor/user/shared-worker"))
  );
  assert_eq!(*observations.register_temp_actor_calls.lock().expect("register_temp_actor_calls"), 1);
  assert_eq!(
    observations.unregister_temp_actor.lock().expect("unregister_temp_actor").clone(),
    Some(String::from("shared-temp"))
  );
  assert_eq!(
    observations.unregister_temp_actor_uri.lock().expect("unregister_temp_actor_uri").clone(),
    Some(temp_path.to_canonical_uri())
  );
}

#[test]
fn actor_ref_provider_handle_shared_from_shared_lock_preserves_inner_type() {
  let observations = ProviderObservations::new();
  let lock = SharedLock::new_with_driver::<DefaultMutex<_>>(ActorRefProviderHandle::new(
    StubActorRefProvider::new(observations),
    SUPPORTED_SCHEMES,
  ));
  let shared = ActorRefProviderHandleShared::from_shared_lock(lock);

  assert_eq!(shared.inner_type_id(), TypeId::of::<StubActorRefProvider>());
}
