use super::SinkRef;
use crate::{dsl::Sink, r#impl::streamref::StreamRefHandoff, materialization::StreamNotUsed};

#[test]
fn into_sink_consumes_sink_ref() {
  let handoff = StreamRefHandoff::<u32>::new();
  let sink_ref = SinkRef::new(handoff);

  let _sink: Sink<u32, StreamNotUsed> = sink_ref.into_sink();
}
