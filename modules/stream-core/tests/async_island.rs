//! Integration tests for Pekko-compatible `async()` island splitting.
//!
//! These tests verify the end-to-end behavior of async boundaries that split
//! a stream graph into independently executed islands, following Pekko semantics.
//!
//! NOTE: These tests will not compile until the production implementation is in place.
//! They define the expected behavioral contract for Gate 0.

mod support;
use fraktor_stream_core_rs::{
  attributes::Attributes,
  dsl::{Flow, Source},
  materialization::StreamNotUsed,
};
use support::RunWithCollectSink;

// --- アイランド境界を越える基本的な要素通過 ---

#[test]
fn async_island_passes_single_element() {
  // 準備: 単一要素の source と async boundary
  // async boundary はグラフを2つのアイランドに分割するが、
  // 観測可能な振る舞いは boundary なしと同一。
  let values =
    Source::single(42_u32).via(Flow::new().r#async()).run_with_collect_sink().expect("run_with_collect_sink");

  // 検証: 要素が通過する
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn async_island_passes_large_sequence() {
  // 準備: アイランド境界チャネル容量を検証するための大きなシーケンス
  let input: Vec<u32> = (0..100).collect();
  let values = Source::from_iterator(input.clone().into_iter())
    .via(Flow::new().r#async())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 全要素が順序通りに到着する
  assert_eq!(values, input);
}

// --- 複数アイランド ---

#[test]
fn two_async_boundaries_create_three_islands() {
  // 準備: 2つの async boundary を持つグラフ → 3アイランド
  let input: Vec<u32> = (1..=10).collect();
  let values = Source::from_iterator(input.clone().into_iter())
    .via(Flow::new().map(|x: u32| x * 2).r#async())
    .via(Flow::new().map(|x: u32| x + 1).r#async())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 両方の変換が適用され、要素が順序通りに到着する
  let expected: Vec<u32> = (1..=10).map(|x| x * 2 + 1).collect();
  assert_eq!(values, expected);
}

// --- 属性付き async boundary ---

#[test]
fn async_with_attributes_passes_elements() {
  // 準備: add_attributes で作成した async boundary 付き flow
  // Pekko のアプローチを模倣: async() は属性を追加するだけ
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(Attributes::async_boundary()))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 要素が正しく通過する
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_with_dispatcher_attribute_passes_elements() {
  // 準備: async boundary + dispatcher 属性付き flow
  let attrs = Attributes::async_boundary().and(Attributes::dispatcher("custom-dispatcher"));
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(attrs))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 要素が正しく通過する（dispatcher は materializer が使用）
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_with_input_buffer_attribute_passes_elements() {
  // 準備: カスタム入力バッファサイズの async boundary
  let attrs = Attributes::async_boundary().and(Attributes::dispatcher("default")).and(Attributes::input_buffer(32, 32));
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(attrs))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 設定されたバッファサイズで要素が通過する
  assert_eq!(values, vec![1_u32, 2, 3]);
}

// --- 完了伝播 ---

#[test]
fn async_island_propagates_normal_completion() {
  // 準備: async boundary を通る有限 source
  let values = Source::from_iterator(vec![1_u32, 2].into_iter())
    .via(Flow::new().r#async())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 全要素出力後にストリームが正常完了する
  assert_eq!(values, vec![1_u32, 2]);
}

#[test]
fn async_island_empty_source_completes() {
  // 準備: async boundary を通る空 source
  let values = Source::<u32, StreamNotUsed>::empty()
    .via(Flow::new().r#async())
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 要素なしでストリームが完了する
  assert!(values.is_empty());
}

// --- 他のオペレーターとの合成 ---

#[test]
fn async_island_with_filter_and_map() {
  // 準備: filter → async → map パイプライン（アイランド境界を越える）
  let values = Source::from_iterator((1_u32..=10).into_iter())
    .via(Flow::new().filter(|x: &u32| x % 2 == 0))
    .via(Flow::new().r#async())
    .via(Flow::new().map(|x: u32| x * 10))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: filter はアイランド1で、map はアイランド2で実行される
  assert_eq!(values, vec![20_u32, 40, 60, 80, 100]);
}

#[test]
fn async_island_with_take() {
  // 準備: source → async → take パイプライン
  let values = Source::from_iterator((1_u32..=100).into_iter())
    .via(Flow::new().r#async())
    .via(Flow::new().take(3))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: 最初の3要素のみが取得される
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_island_with_grouped() {
  // 準備: source → async → grouped パイプライン
  let values = Source::from_iterator((1_u32..=6).into_iter())
    .via(Flow::new().r#async())
    .via(Flow::new().grouped(2).expect("grouped"))
    .run_with_collect_sink()
    .expect("run_with_collect_sink");

  // 検証: アイランド境界を越えた後、要素がペアにグループ化される
  assert_eq!(values, vec![vec![1_u32, 2], vec![3, 4], vec![5, 6]]);
}
