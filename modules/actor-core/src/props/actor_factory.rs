use cellactor_utils_core_rs::sync::ArcShared;

use crate::actor::Actor;

/// Trait implemented by actor factories stored inside [`Props`](super::props_struct::Props).
pub trait ActorFactory: Send + Sync {
  /// Creates a new actor instance wrapped in [`ArcShared`].
  fn create(&self) -> ArcShared<dyn Actor + Send + Sync>;
}

impl<F, A> ActorFactory for F
where
  F: Fn() -> A + Send + Sync + 'static,
  A: Actor + Sync + 'static,
{
  fn create(&self) -> ArcShared<dyn Actor + Send + Sync> {
    let actor = (self)();
    ArcShared::new(actor)
  }
}
