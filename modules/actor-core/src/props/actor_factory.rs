use alloc::boxed::Box;

use crate::actor::Actor;

/// Trait implemented by actor factories stored inside [`Props`](super::props_struct::Props).
pub trait ActorFactory: Send + Sync {
  /// Creates a new actor instance boxed behind a trait object.
  fn create(&self) -> Box<dyn Actor + Send + Sync>;
}

impl<F, A> ActorFactory for F
where
  F: Fn() -> A + Send + Sync + 'static,
  A: Actor + Sync + 'static,
{
  fn create(&self) -> Box<dyn Actor + Send + Sync> {
    Box::new((self)())
  }
}
