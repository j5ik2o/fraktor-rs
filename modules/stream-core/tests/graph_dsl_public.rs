use fraktor_stream_core_rs::core::{
  StreamError,
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source},
  materialization::{KeepLeft, KeepRight, StreamNotUsed},
  shape::{Inlet, Outlet},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

#[test]
fn graph_dsl_is_public_and_builds_linear_flow() {
  // Given: crate public API から GraphDsl facade を使う
  let flow =
    GraphDsl::builder::<u32>().via(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value.saturating_mul(2))).build();

  // When: Source に接続して実行する
  let values = Source::single(21_u32).via(flow).collect_values().expect("collect_values");

  // Then: public facade 経由の graph が data path として機能する
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn graph_dsl_from_flow_is_public_and_builds_external_crate_flow() {
  // Given: public GraphDsl::from_flow で既存 flow を builder に取り込む
  let builder = GraphDsl::from_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 10));
  let flow = builder.build();

  // When: 外部 crate 視点で Source に接続する
  let values = Source::single(5_u32).via(flow).collect_values().expect("collect_values");

  // Then: from_flow 経由の graph が data path として実行される
  assert_eq!(values, vec![15_u32]);
}

#[test]
fn graph_dsl_create_flow_is_public_and_uses_builder_block() {
  // Given: create_flow の builder block で Flow を追加する
  let flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 5)).expect("add_flow");
  });

  // When: public Source から実行する
  let values = Source::single(3_u32).via(flow).collect_values().expect("collect_values");

  // Then: builder block 内の graph が実行結果に反映される
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn graph_dsl_create_flow_mat_is_public_and_preserves_materialized_value() {
  // Given: create_flow_mat に明示的な materialized value を渡す
  let flow = GraphDsl::create_flow_mat(17_u32, |builder: &mut GraphDslBuilder<u32, u32, u32>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1)).expect("add_flow");
  });

  // When: Source 側から flow の materialized value を保持する
  let graph = Source::single(1_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  // Then: public API 経由で materialized value が観測できる
  assert_eq!(graph.materialized(), &17_u32);
}

#[test]
fn graph_dsl_create_source_is_public_and_uses_explicit_wiring() {
  // Given: builder に Source と Flow を追加して明示的に接続する
  let source = GraphDsl::create_source(|builder: &mut GraphDslBuilder<(), u32, StreamNotUsed>| {
    let outlet = builder.add_source(Source::single(4_u32)).expect("add_source");
    let downstream =
      builder.wire_via(&outlet, Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 6)).expect("wire_via");
    let _ = downstream.id();
  });

  // When: public Source として実行する
  let values = source.collect_values().expect("collect_values");

  // Then: 明示的な wiring が data path に反映される
  assert_eq!(values, vec![10_u32]);
}

#[test]
fn graph_dsl_create_sink_is_public_and_builds_external_crate_sink() {
  // Given: create_sink の builder block で public Sink を追加する
  let observed = ArcShared::new(SpinSyncMutex::new(Vec::<u32>::new()));
  let observed_clone = observed.clone();
  let sink = GraphDsl::create_sink(|builder: &mut GraphDslBuilder<u32, (), StreamNotUsed>| {
    builder.add_sink(Sink::<u32, _>::foreach(move |value| observed_clone.lock().push(value))).expect("add_sink");
  });

  // When: public Source から作成済み Sink に到達させる
  let values = Source::from_array([1_u32, 2, 3]).also_to(sink).collect_values().expect("collect_values");

  // Then: create_sink 経由の sink へ要素が配送される
  assert_eq!(values, vec![1_u32, 2, 3]);
  assert_eq!(*observed.lock(), vec![1_u32, 2, 3]);
}

#[test]
fn graph_dsl_builder_public_connect_rejects_unknown_ports() {
  // Given: builder に登録されていない port
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let outlet = Outlet::<u32>::new();
  let inlet = Inlet::<u32>::new();

  // When: 未登録 port を接続する
  let result = builder.connect(&outlet, &inlet);

  // Then: public API は不正な GraphDSL 利用を InvalidConnection として返す
  assert_eq!(result, Err(StreamError::InvalidConnection));
}
