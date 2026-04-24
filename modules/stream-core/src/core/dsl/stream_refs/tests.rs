use super::StreamRefs;
use crate::core::{
  dsl::{Sink, Source},
  stream_ref::{SinkRef, SourceRef},
};

#[test]
fn source_ref_returns_sink_materializing_source_ref() {
  let _sink: Sink<u32, SourceRef<u32>> = StreamRefs::source_ref();
}

#[test]
fn sink_ref_returns_source_materializing_sink_ref() {
  let _source: Source<u32, SinkRef<u32>> = StreamRefs::sink_ref();
}
