use crate::core::{
  StreamError,
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source},
  materialization::{KeepLeft, KeepRight, StreamNotUsed},
  shape::{Inlet, Outlet},
};

#[test]
fn create_flow_builds_executable_flow_from_public_builder_block() {
  // Given: GraphDsl::create_flow で map stage を public builder に追加する
  let flow = GraphDsl::create_flow(|builder: &mut GraphDslBuilder<u32, u32, StreamNotUsed>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value.saturating_mul(3))).expect("add_flow");
  });

  // When: 利用者 API から Source に接続して実行する
  let values = Source::single(4_u32).via(flow).collect_values().expect("collect_values");

  // Then: builder block で追加した flow が data path に反映される
  assert_eq!(values, vec![12_u32]);
}

#[test]
fn create_flow_mat_preserves_initial_materialized_value() {
  // Given: create_flow_mat に明示的な materialized value を渡す
  let flow = GraphDsl::create_flow_mat(99_u32, |builder: &mut GraphDslBuilder<u32, u32, u32>| {
    builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1)).expect("add_flow");
  });

  // When: KeepRight で flow 側の materialized value を保持する
  let graph = Source::single(1_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  // Then: public facade は内部 graph builder の mat を捨てない
  assert_eq!(graph.materialized(), &99_u32);
}

#[test]
fn create_source_uses_explicit_builder_wiring() {
  // Given: source graph を追加し、wire_via で downstream flow へ接続する
  let source = GraphDsl::create_source(|builder: &mut GraphDslBuilder<(), u32, StreamNotUsed>| {
    let outlet = builder.add_source(Source::single(5_u32)).expect("add_source");
    let outlet =
      builder.wire_via(&outlet, Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 7)).expect("wire_via");
    let _ = outlet.id();
  });

  // When: public Source として実行する
  let values = source.collect_values().expect("collect_values");

  // Then: builder 内の明示的な接続順に値が流れる
  assert_eq!(values, vec![12_u32]);
}

#[test]
fn create_sink_returns_sink_that_can_be_connected_from_public_source() {
  // Given: Sink graph 内で flow -> sink を明示的に接続する
  let sink = GraphDsl::create_sink(|builder: &mut GraphDslBuilder<u32, (), StreamNotUsed>| {
    let (flow_in, flow_out) =
      builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1)).expect("add_flow");
    let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).expect("add_sink");
    builder.connect(&flow_out, &sink_in).expect("connect flow to sink");
    let _ = flow_in.id();
  });

  // When: public Source から Sink に接続する
  let graph = Source::single(41_u32).into_mat(sink, KeepRight);

  // Then: create_sink の戻り値は public RunnableGraph へ到達できる
  assert_eq!(graph.materialized(), &StreamNotUsed::new());
}

#[test]
fn builder_connect_rejects_ports_not_added_to_builder() {
  // Given: builder に登録されていない port
  let mut builder = GraphDsl::builder::<u32>();
  let outlet = Outlet::<u32>::new();
  let inlet = Inlet::<u32>::new();

  // When: 未登録 port 同士を接続する
  let result = builder.connect(&outlet, &inlet);

  // Then: Pekko の不正 builder 利用検証と同じく失敗として観測できる
  assert_eq!(result, Err(StreamError::InvalidConnection));
}
