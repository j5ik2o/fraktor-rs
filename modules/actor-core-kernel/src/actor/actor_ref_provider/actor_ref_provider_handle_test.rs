use alloc::string::String;
use std::sync::{Arc, Mutex};

use super::ActorRefProviderHandle;
use crate::{
  actor::{
    Address, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, NullSender},
    actor_ref_provider::ActorRefProvider,
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
  unregister_temp_actor:     Arc<Mutex<Option<String>>>,
  unregister_temp_actor_uri: Arc<Mutex<Option<String>>>,
}

impl ProviderObservations {
  fn new() -> Self {
    Self {
      actor_ref_path:            Arc::new(Mutex::new(None)),
      resolve_actor_ref_path:    Arc::new(Mutex::new(None)),
      resolve_actor_ref_str:     Arc::new(Mutex::new(None)),
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
    ActorRef::with_canonical_path(Pid::new(pid, 0), NullSender, ActorPath::root().child("stub"))
  }
}

impl ActorRefProvider for StubActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    SUPPORTED_SCHEMES
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    *self.observations.actor_ref_path.lock().expect("actor_ref_path") = Some(path.to_canonical_uri());
    Ok(Self::actor_ref_with_pid(10))
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(11))
  }

  fn guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(12))
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(13))
  }

  fn dead_letters(&self) -> ActorRef {
    Self::actor_ref_with_pid(14)
  }

  fn temp_path(&self) -> ActorPath {
    ActorPath::root().child("temp")
  }

  fn root_path(&self) -> ActorPath {
    ActorPath::root()
  }

  fn root_guardian_at(&self, _address: &Address) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(15))
  }

  fn deployer(&self) -> Option<Deployer> {
    let mut deployer = Deployer::new();
    deployer.register("/user/stub", Deploy::new());
    Some(deployer)
  }

  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    *self.observations.resolve_actor_ref_path.lock().expect("resolve_actor_ref_path") = Some(path.to_canonical_uri());
    Ok(Self::actor_ref_with_pid(16))
  }

  fn resolve_actor_ref_str(&mut self, path: &str) -> Result<ActorRef, ActorError> {
    *self.observations.resolve_actor_ref_str.lock().expect("resolve_actor_ref_str") = Some(String::from(path));
    Ok(Self::actor_ref_with_pid(17))
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    Ok(ActorPath::root().child("temp").child(prefix))
  }

  fn temp_container(&self) -> Option<ActorRef> {
    Some(Self::actor_ref_with_pid(18))
  }

  fn register_temp_actor(&self, _actor: ActorRef) -> Option<String> {
    Some(String::from("registered-temp"))
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
    Some(Self::actor_ref_with_pid(19))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }

  fn get_external_address_for(&self, _addr: &Address) -> Option<Address> {
    Some(Address::remote("stub-system", "127.0.0.1", 2552))
  }

  fn get_default_address(&self) -> Option<Address> {
    Some(Address::local("stub-system"))
  }
}

#[test]
fn actor_ref_provider_handle_delegates_all_provider_methods() {
  let observations = ProviderObservations::new();
  let provider = StubActorRefProvider::new(observations.clone());
  let mut handle = ActorRefProviderHandle::new(provider, SUPPORTED_SCHEMES);
  let actor_path = ActorPath::root().child("worker");
  let temp_path = ActorPath::root().child("temp").child("registered-temp");

  assert_eq!(ActorRefProvider::supported_schemes(&handle), SUPPORTED_SCHEMES);
  assert_eq!(handle.actor_ref(actor_path.clone()).expect("actor_ref").pid(), Pid::new(10, 0));
  assert_eq!(handle.root_guardian().expect("root_guardian").pid(), Pid::new(11, 0));
  assert_eq!(handle.guardian().expect("guardian").pid(), Pid::new(12, 0));
  assert_eq!(handle.system_guardian().expect("system_guardian").pid(), Pid::new(13, 0));
  assert_eq!(handle.dead_letters().pid(), Pid::new(14, 0));
  assert_eq!(handle.temp_path().to_canonical_uri(), "fraktor://cellactor/user/temp");
  assert_eq!(handle.root_path().to_canonical_uri(), "fraktor://cellactor/user");
  assert_eq!(handle.root_guardian_at(&Address::local("stub-system")).expect("root_guardian_at").pid(), Pid::new(15, 0));
  assert!(handle.deployer().expect("deployer").deploy_for("/user/stub").is_some());
  assert_eq!(handle.resolve_actor_ref(actor_path.clone()).expect("resolve_actor_ref").pid(), Pid::new(16, 0));
  assert_eq!(
    handle.resolve_actor_ref_str("fraktor://cellactor/user/worker").expect("resolve_actor_ref_str").pid(),
    Pid::new(17, 0)
  );
  assert_eq!(
    handle.temp_path_with_prefix("registered-temp").expect("temp_path_with_prefix").to_canonical_uri(),
    temp_path.to_canonical_uri()
  );
  assert_eq!(handle.temp_container().expect("temp_container").pid(), Pid::new(18, 0));
  assert_eq!(handle.register_temp_actor(ActorRef::null()), Some(String::from("registered-temp")));
  handle.unregister_temp_actor("registered-temp");
  assert!(handle.unregister_temp_actor_path(&temp_path).is_ok());
  assert_eq!(handle.temp_actor("registered-temp").expect("temp_actor").pid(), Pid::new(19, 0));
  assert!(handle.termination_signal().is_terminated());
  assert_eq!(
    handle.get_external_address_for(&Address::local("stub-system")).expect("get_external_address_for").to_uri_string(),
    "fraktor.tcp://stub-system@127.0.0.1:2552"
  );
  assert_eq!(handle.get_default_address().expect("get_default_address").to_uri_string(), "fraktor://stub-system");

  assert_eq!(observations.actor_ref_path.lock().expect("actor_ref_path").clone(), Some(actor_path.to_canonical_uri()));
  assert_eq!(
    observations.resolve_actor_ref_path.lock().expect("resolve_actor_ref_path").clone(),
    Some(actor_path.to_canonical_uri())
  );
  assert_eq!(
    observations.resolve_actor_ref_str.lock().expect("resolve_actor_ref_str").clone(),
    Some(String::from("fraktor://cellactor/user/worker"))
  );
  assert_eq!(
    observations.unregister_temp_actor.lock().expect("unregister_temp_actor").clone(),
    Some(String::from("registered-temp"))
  );
  assert_eq!(
    observations.unregister_temp_actor_uri.lock().expect("unregister_temp_actor_uri").clone(),
    Some(temp_path.to_canonical_uri())
  );
}
