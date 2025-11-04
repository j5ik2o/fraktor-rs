use alloc::boxed::Box;

#[cfg(test)]
mod tests;

use crate::{NoStdToolbox, RuntimeToolbox, actor_prim::Actor};

/// Trait implemented by actor factories stored inside [`Props`](super::base::Props).
pub trait ActorFactory<TB: RuntimeToolbox = NoStdToolbox>: Send + Sync {
  /// Creates a new actor instance boxed behind a trait object.
  fn create(&self) -> Box<dyn Actor<TB> + Send + Sync>;
}

impl<F, A, TB> ActorFactory<TB> for F
where
  F: Fn() -> A + Send + Sync + 'static,
  A: Actor<TB> + Sync + 'static,
  TB: RuntimeToolbox,
{
  fn create(&self) -> Box<dyn Actor<TB> + Send + Sync> {
    Box::new((self)())
  }
}
