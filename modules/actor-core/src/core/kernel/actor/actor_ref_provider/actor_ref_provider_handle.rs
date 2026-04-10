//! Handle wrapper for ActorRefProvider implementations.

use alloc::string::String;

use super::ActorRefProvider;
use crate::core::kernel::{
  actor::{
    Address,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
    deploy::Deployer,
    error::ActorError,
  },
  system::TerminationSignal,
};

/// Handle wrapper that combines a provider with its supported schemes.
///
/// This struct stores a static reference to the supported schemes, avoiding
/// repeated calls to `supported_schemes()` on the inner provider.
pub struct ActorRefProviderHandle<P> {
  provider: P,
  schemes:  &'static [ActorPathScheme],
}

impl<P> ActorRefProviderHandle<P> {
  pub(crate) const fn new(provider: P, schemes: &'static [ActorPathScheme]) -> Self {
    Self { provider, schemes }
  }

  const fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.schemes
  }
}

impl<P> ActorRefProvider for ActorRefProviderHandle<P>
where
  P: ActorRefProvider + 'static,
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.supported_schemes()
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.provider.actor_ref(path)
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    self.provider.root_guardian()
  }

  fn guardian(&self) -> Option<ActorRef> {
    self.provider.guardian()
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    self.provider.system_guardian()
  }

  fn dead_letters(&self) -> ActorRef {
    self.provider.dead_letters()
  }

  fn temp_path(&self) -> ActorPath {
    self.provider.temp_path()
  }

  fn root_path(&self) -> ActorPath {
    self.provider.root_path()
  }

  fn root_guardian_at(&self, address: &Address) -> Option<ActorRef> {
    self.provider.root_guardian_at(address)
  }

  fn deployer(&self) -> Option<Deployer> {
    self.provider.deployer()
  }

  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.provider.resolve_actor_ref(path)
  }

  fn resolve_actor_ref_str(&mut self, path: &str) -> Result<ActorRef, ActorError> {
    self.provider.resolve_actor_ref_str(path)
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    self.provider.temp_path_with_prefix(prefix)
  }

  fn temp_container(&self) -> Option<ActorRef> {
    self.provider.temp_container()
  }

  fn register_temp_actor(&self, actor: ActorRef) -> Option<String> {
    self.provider.register_temp_actor(actor)
  }

  fn unregister_temp_actor(&self, name: &str) {
    self.provider.unregister_temp_actor(name);
  }

  fn unregister_temp_actor_path(&self, path: &ActorPath) -> Result<(), ActorError> {
    self.provider.unregister_temp_actor_path(path)
  }

  fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.provider.temp_actor(name)
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.provider.termination_signal()
  }

  fn get_external_address_for(&self, addr: &Address) -> Option<Address> {
    self.provider.get_external_address_for(addr)
  }

  fn get_default_address(&self) -> Option<Address> {
    self.provider.get_default_address()
  }
}
