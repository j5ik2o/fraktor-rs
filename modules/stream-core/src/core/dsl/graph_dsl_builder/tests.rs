use crate::core::{
  StreamError,
  dsl::{Flow, GraphDslBuilder, Sink, Source},
  materialization::{KeepBoth, KeepLeft, KeepRight, StreamNotUsed},
  shape::{Inlet, Outlet},
};

#[test]
fn builder_new_via_build_creates_linear_flow() {
  // Given: public builder に linear flow を追加する
  let flow = GraphDslBuilder::<u32, u32, StreamNotUsed>::new()
    .via(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1))
    .build();

  // When: Source から実行する
  let values = Source::single(1_u32).via(flow).collect_values().expect("collect_values");

  // Then: builder で追加した flow が適用される
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn builder_via_mat_combines_materialized_values() {
  // Given: builder と downstream flow の materialized value を KeepBoth で結合する
  let flow = GraphDslBuilder::<u32, u32, u32>::from_flow(
    Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1).map_materialized_value(|_| 7_u32),
  )
  .via_mat(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value * 2).map_materialized_value(|_| 11_u32), KeepBoth)
  .build();

  // When: Source 側から flow の materialized value を保持する
  let graph = Source::single(2_u32).via_mat(flow, KeepRight).into_mat(Sink::<u32, _>::ignore(), KeepLeft);

  // Then: public builder は内部 mat combine rule を公開 API 経由で保持する
  assert_eq!(graph.materialized(), &(7_u32, 11_u32));
}

#[test]
fn add_source_mat_returns_imported_outlet_and_materialized_value() {
  // Given: materialized value を持つ Source
  let source = Source::single(10_u32).map_materialized_value(|_| 23_u32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: public builder に Source を追加する
  let (outlet, materialized) = builder.add_source_mat(source).expect("add_source_mat");

  // Then: port と materialized value の両方が取得できる
  let _ = outlet.id();
  assert_eq!(materialized, 23_u32);
}

#[test]
fn add_flow_mat_returns_imported_ports_and_materialized_value() {
  // Given: materialized value を持つ Flow
  let flow = Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1).map_materialized_value(|_| 31_u32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: public builder に Flow を追加する
  let (inlet, outlet, materialized) = builder.add_flow_mat(flow).expect("add_flow_mat");

  // Then: inlet/outlet と materialized value が保持される
  let _ = inlet.id();
  let _ = outlet.id();
  assert_eq!(materialized, 31_u32);
}

#[test]
fn add_sink_mat_returns_imported_inlet_and_materialized_value() {
  // Given: materialized value を持つ Sink
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 47_u32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: public builder に Sink を追加する
  let (inlet, materialized) = builder.add_sink_mat(sink).expect("add_sink_mat");

  // Then: inlet と materialized value が保持される
  let _ = inlet.id();
  assert_eq!(materialized, 47_u32);
}

#[test]
fn connect_rejects_unknown_ports() {
  // Given: builder に追加されていない port
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let outlet = Outlet::<u32>::new();
  let inlet = Inlet::<u32>::new();

  // When: 未登録 port を接続する
  let result = builder.connect(&outlet, &inlet);

  // Then: InvalidConnection として fail fast する
  assert_eq!(result, Err(StreamError::InvalidConnection));
}
