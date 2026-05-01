use fraktor_stream_core_rs::core::{
  StreamError,
  dsl::{Sink, Source},
  materialization::{KeepLeft, KeepRight, StreamDone, StreamFuture, StreamNotUsed},
  stage::{CancellationCause, SubSinkInlet, SubSinkInletHandler, SubSourceOutlet, SubSourceOutletHandler},
};

struct PublicSubSinkInletHandler;

impl SubSinkInletHandler<u32> for PublicSubSinkInletHandler {
  fn on_push(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

struct PublicSubSourceOutletHandler;

impl SubSourceOutletHandler<u32> for PublicSubSourceOutletHandler {
  fn on_pull(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

#[test]
fn sub_sink_inlet_is_public_and_exposes_sink_endpoint() {
  // Given: crate public API から SubSinkInlet を作成する
  let mut inlet = SubSinkInlet::<u32>::new("public-sub-sink");
  inlet.set_handler(PublicSubSinkInletHandler);
  let sink = inlet.sink();

  // When: public Source から SubSinkInlet.sink に接続する
  let graph = Source::single(1_u32).into_mat(sink, KeepRight);

  // Then: SubSinkInlet.sink は NotUsed materialized value の Sink として公開される
  assert_eq!(graph.materialized(), &StreamNotUsed::new());
}

#[test]
fn sub_source_outlet_is_public_and_exposes_source_endpoint() {
  // Given: crate public API から SubSourceOutlet を作成する
  let mut outlet = SubSourceOutlet::<u32>::new("public-sub-source");
  outlet.set_handler(PublicSubSourceOutletHandler);
  let source = outlet.source();

  // When: public Sink に接続し Source 側の materialized value を保持する
  let graph = source.into_mat(Sink::<u32, StreamFuture<StreamDone>>::ignore(), KeepLeft);

  // Then: SubSourceOutlet.source は NotUsed materialized value の Source として公開される
  assert_eq!(graph.materialized(), &StreamNotUsed::new());
}

#[test]
fn sub_sink_inlet_handler_default_failure_contract_is_public() {
  // Given: public handler trait を最小実装した型
  let mut handler = PublicSubSinkInletHandler;

  // When: upstream failure default callback を呼び出す
  let result = handler.on_upstream_failure(StreamError::Failed);

  // Then: Pekko InHandler と同じく failure が伝播される
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn sub_source_outlet_handler_default_cancellation_contract_is_public() {
  // Given: public handler trait を最小実装した型
  let mut handler = PublicSubSourceOutletHandler;

  // When: downstream finish default callback を呼び出す
  let result = handler.on_downstream_finish(CancellationCause::no_more_elements_needed());

  // Then: Pekko OutHandler と同じく default は cancellation を吸収せず正常終了する
  assert_eq!(result, Ok(()));
}
