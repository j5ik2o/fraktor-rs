use super::SourceRef;
use crate::{dsl::Source, r#impl::streamref::StreamRefHandoff, materialization::StreamNotUsed};

#[test]
fn into_source_consumes_source_ref() {
  let handoff = StreamRefHandoff::<u32>::new();
  let source_ref = SourceRef::new(handoff);

  let _source: Source<u32, StreamNotUsed> = source_ref.into_source();
}
