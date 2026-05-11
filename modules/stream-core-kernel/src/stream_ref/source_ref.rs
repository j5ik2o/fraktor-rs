#[cfg(test)]
#[path = "source_ref_test.rs"]
mod tests;

use core::marker::PhantomData;

use crate::{
  dsl::Source,
  r#impl::streamref::{StreamRefHandoff, StreamRefSourceLogic},
  materialization::StreamNotUsed,
  stage::StageKind,
};

/// Reference to a source side of a stream reference.
pub struct SourceRef<T> {
  handoff: StreamRefHandoff<T>,
  _pd:     PhantomData<fn() -> T>,
}

impl<T> SourceRef<T> {
  pub(crate) const fn new(handoff: StreamRefHandoff<T>) -> Self {
    Self { handoff, _pd: PhantomData }
  }
}

impl<T> SourceRef<T>
where
  T: Send + Sync + 'static,
{
  /// Converts this reference into the source it points to.
  #[must_use]
  pub fn into_source(self) -> Source<T, StreamNotUsed> {
    self.handoff.subscribe();
    Source::from_logic(StageKind::Custom, StreamRefSourceLogic::subscribed(self.handoff))
  }
}
