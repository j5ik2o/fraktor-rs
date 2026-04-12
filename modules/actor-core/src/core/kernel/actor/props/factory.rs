use alloc::boxed::Box;

#[cfg(test)]
mod tests;

use crate::core::kernel::actor::Actor;

/// Trait implemented by actor factories stored inside [`Props`](super::base::Props).
pub trait ActorFactory: Send + Sync {
  /// Creates a new actor instance boxed behind a trait object.
  fn create(&mut self) -> Box<dyn Actor + Send>;
}

impl<F, A> ActorFactory for F
where
  F: FnMut() -> A + Send + Sync + 'static,
  A: Actor + 'static,
{
  fn create(&mut self) -> Box<dyn Actor + Send> {
    Box::new((self)())
  }
}
