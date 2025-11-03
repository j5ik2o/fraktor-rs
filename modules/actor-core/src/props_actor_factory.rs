use alloc::boxed::Box;

use crate::{NoStdToolbox, RuntimeToolbox, actor::Actor};

/// Trait implemented by actor factories stored inside [`Props`](super::props_struct::Props).
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
