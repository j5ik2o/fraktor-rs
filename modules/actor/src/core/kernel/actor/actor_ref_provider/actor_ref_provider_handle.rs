//! Handle wrapper for ActorRefProvider implementations.

use super::ActorRefProvider;
use crate::core::kernel::actor::{
  actor_path::{ActorPath, ActorPathScheme},
  actor_ref::ActorRef,
  error::ActorError,
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

  /// Returns a reference to the inner provider.
  ///
  /// This method is intended for testing and debugging purposes only.
  #[doc(hidden)]
  pub const fn inner(&self) -> &P {
    &self.provider
  }

  /// Returns a mutable reference to the inner provider.
  ///
  /// This method is intended for testing and debugging purposes only.
  #[doc(hidden)]
  pub const fn inner_mut(&mut self) -> &mut P {
    &mut self.provider
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
}
