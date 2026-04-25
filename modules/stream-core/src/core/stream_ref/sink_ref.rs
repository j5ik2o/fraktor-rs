#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use crate::core::{
  dsl::Sink,
  r#impl::streamref::{StreamRefHandoff, StreamRefSinkLogic},
  materialization::StreamNotUsed,
  stage::StageKind,
};

/// Reference to a sink side of a stream reference.
pub struct SinkRef<T> {
  handoff: StreamRefHandoff<T>,
  _pd:     PhantomData<fn(T)>,
}

impl<T> SinkRef<T> {
  pub(in crate::core) const fn new(handoff: StreamRefHandoff<T>) -> Self {
    Self { handoff, _pd: PhantomData }
  }
}

impl<T> SinkRef<T>
where
  T: Send + Sync + 'static,
{
  /// Converts this reference into the sink it points to.
  #[must_use]
  pub fn into_sink(self) -> Sink<T, StreamNotUsed> {
    self.handoff.subscribe();
    let logic = StreamRefSinkLogic::subscribed(self.handoff, None);
    Sink::from_logic(StageKind::Custom, logic)
  }
}
