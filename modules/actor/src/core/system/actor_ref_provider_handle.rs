//! Handle wrapper for ActorRefProvider implementations.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::ActorRefProvider;
use crate::core::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
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

impl<TB, P> ActorRefProvider<TB> for ActorRefProviderHandle<P>
where
  TB: RuntimeToolbox + 'static,
  P: ActorRefProvider<TB> + 'static,
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.supported_schemes()
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.provider.actor_ref(path)
  }
}
