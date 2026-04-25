#[cfg(test)]
mod tests;

use super::{Sink, Source, StageKind, StreamNotUsed};
use crate::core::{
  r#impl::streamref::{StreamRefHandoff, StreamRefSinkLogic, StreamRefSourceLogic},
  stream_ref::{SinkRef, SourceRef},
};

/// Factory namespace for stream reference endpoints.
pub struct StreamRefs;

impl StreamRefs {
  /// Creates a local sink materializing a source reference.
  #[must_use]
  pub fn source_ref<T>() -> Sink<T, SourceRef<T>>
  where
    T: Send + Sync + 'static, {
    let handoff = StreamRefHandoff::new();
    let source_ref = SourceRef::new(handoff.clone());
    let logic = StreamRefSinkLogic::awaiting_remote_subscription(handoff);
    Sink::from_logic(StageKind::Custom, logic).map_materialized_value(|_| source_ref)
  }

  /// Creates a local source materializing a sink reference.
  #[must_use]
  pub fn sink_ref<T>() -> Source<T, SinkRef<T>>
  where
    T: Send + Sync + 'static, {
    let handoff = StreamRefHandoff::new();
    let sink_ref = SinkRef::new(handoff.clone());
    let logic = StreamRefSourceLogic::awaiting_remote_subscription(handoff);
    Source::<T, StreamNotUsed>::from_logic(StageKind::Custom, logic).map_materialized_value(|_| sink_ref)
  }
}
