use crate::core::{
  StreamError, StreamNotUsed,
  dsl::{Flow, Sink, Source},
  graph::{GraphDslBuilder, ReversePortOps},
  shape::Inlet,
};

// --- ReversePortOps の構築 ---

#[test]
fn reverse_port_ops_new_wraps_inlet() {
  // 前提: inlet がある
  let inlet = Inlet::<u32>::new();

  // 実行: ReversePortOps で包む
  let ops = ReversePortOps::new(&inlet);

  // 検証: inlet を取り出せる
  assert_eq!(ops.inlet(), inlet);
}

#[test]
fn reverse_port_ops_from_inlet_conversion() {
  // 前提: inlet がある
  let inlet = Inlet::<u32>::new();

  // 実行: From trait で変換する
  let ops: ReversePortOps<u32> = ReversePortOps::from(inlet);

  // 検証: inlet が一致する
  assert_eq!(ops.inlet(), inlet);
}

#[test]
fn reverse_port_ops_is_copy() {
  // 前提: ReversePortOps がある
  let inlet = Inlet::<u32>::new();
  let ops = ReversePortOps::new(&inlet);

  // 実行: Copy で複製する
  let ops2 = ops;
  let _ops3 = ops; // Copy ならコンパイルできる

  // 検証: 両方のコピーが有効
  assert_eq!(ops2.inlet(), ops.inlet());
}

// --- ReversePortOps::from_source の検証 ---

#[test]
fn reverse_port_ops_from_source_connects_source_to_inlet() -> Result<(), StreamError> {
  // 前提: sink を追加して inlet を得た builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;

  // 実行: ReversePortOps で source を inlet へ接続する
  let result = ReversePortOps::new(&sink_in).from_source(Source::single(7_u32), &mut builder);

  // 検証: 接続と plan 構築が成功する
  assert!(result.is_ok());
  assert!(builder.build().into_parts().0.into_plan().is_ok());
  Ok(())
}

#[test]
fn reverse_port_ops_from_source_with_flow_inlet() -> Result<(), StreamError> {
  // 前提: flow と sink を接続済みの builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let (flow_in, flow_out) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1))?;
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;
  builder.connect(&flow_out, &sink_in)?;

  // 実行: ReversePortOps で source を flow の inlet へ接続する
  let result = ReversePortOps::new(&flow_in).from_source(Source::single(10_u32), &mut builder);

  // 検証: 接続と graph 構築が成功する
  assert!(result.is_ok());
  assert!(builder.build().into_parts().0.into_plan().is_ok());
  Ok(())
}

// --- ReversePortOps::connect_from の検証 ---

#[test]
fn reverse_port_ops_connect_from_connects_outlet_to_inlet() -> Result<(), StreamError> {
  // 前提: source と sink を持つ builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(42_u32))?;
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;

  // 実行: ReversePortOps::connect_from を使う
  let result = ReversePortOps::new(&sink_in).connect_from(&source_out, &mut builder);

  // 検証: 接続と plan 構築が成功する
  assert!(result.is_ok());
  assert!(builder.build().into_parts().0.into_plan().is_ok());
  Ok(())
}
