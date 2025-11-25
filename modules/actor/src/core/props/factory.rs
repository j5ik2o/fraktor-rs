use alloc::boxed::Box;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

#[cfg(test)]
mod tests;

use crate::core::actor_prim::Actor;

/// Trait implemented by actor factories stored inside [`Props`](super::base::Props).
pub trait ActorFactory<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync {
  /// Creates a new actor instance boxed behind a trait object.
  fn create(&mut self) -> Box<dyn Actor<TB> + Send + Sync>;
}

impl<F, A, TB> ActorFactory<TB> for F
where
  F: FnMut() -> A + Send + Sync + 'static,
  A: Actor<TB> + Sync + 'static,
  TB: RuntimeToolbox,
{
  fn create(&mut self) -> Box<dyn Actor<TB> + Send + Sync> {
    Box::new((self)())
  }
}
