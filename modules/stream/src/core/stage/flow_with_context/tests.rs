use alloc::{vec, vec::Vec};

use crate::core::{
  StreamNotUsed,
  stage::{FlowWithContext, Source, flow::Flow},
};

#[test]
fn should_map_output_preserving_context() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v)).map(|s: &str| s.len());
  let values = Source::from(vec![(1_i32, "hello"), (2, "world")]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v)).filter(|v: &i32| *v > 0);
  let values = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  // Ctx=i32, Ctx2=i64 — 型が異なることで map_context が恒等でないことを保証
  let fwc: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  // forward と reverse は逆関数ではない: 出力コンテキストは入力と異なる
  let mapped = fwc.map_context(|ctx: i32| i64::from(ctx) * 10, |ctx2: i64| (ctx2 as i32) - 1);
  // 入力: (10_i64, "a"), (20_i64, "b")
  // → reverse(10) = 9, reverse(20) = 19
  // → inner (恒等): (9, "a"), (19, "b")
  // → forward(9) = 90, forward(19) = 190
  let values = Source::from(vec![(10_i64, "a"), (20_i64, "b")]).via(mapped.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(90_i64, "a"), (190_i64, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc1: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let fwc2: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let composed = fwc1.via(fwc2);
  let values = Source::from(vec![(1_i32, "hello"), (2, "hi")]).via(composed.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}

// --- map_concat テスト ---

#[test]
fn map_concat_expands_elements_preserving_context() {
  // 準備: 各文字列を文字に展開する FlowWithContext
  let fwc: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let expanded = fwc.map_concat(|s: &str| s.chars().map(|c| c as u32).collect::<Vec<_>>());

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, "ab"), (2, "c")]).via(expanded.as_flow()).collect_values().unwrap();

  // 検証: 展開された各要素は元のコンテキストを保持
  assert_eq!(values, vec![(1, 97), (1, 98), (2, 99)]);
}

#[test]
fn map_concat_empty_expansion_drops_element() {
  // 準備: 一部の要素で空のイテレータを返す FlowWithContext
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let expanded = fwc.map_concat(|v: i32| if v > 0 { vec![v, v * 10] } else { vec![] });

  // 実行: 空イテレータを生成する要素を含めて流す
  let values = Source::from(vec![(1_i32, 5), (2, -1), (3, 3)]).via(expanded.as_flow()).collect_values().unwrap();

  // 検証: 空展開の要素は除外、それ以外は同じコンテキストで展開
  assert_eq!(values, vec![(1, 5), (1, 50), (3, 3), (3, 30)]);
}

// --- filter_not テスト ---

#[test]
fn filter_not_passes_elements_where_predicate_is_false() {
  // 準備: 正の値を除外する FlowWithContext
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let filtered = fwc.filter_not(|v: &i32| *v > 0);

  // 実行: 要素を流す
  let values =
    Source::from(vec![(1_i32, 10), (2, -5), (3, 0), (4, 20)]).via(filtered.as_flow()).collect_values().unwrap();

  // 検証: 述語が false の要素のみ通過、コンテキスト保持
  assert_eq!(values, vec![(2, -5), (3, 0)]);
}

#[test]
fn filter_not_passes_all_when_predicate_always_false() {
  // 準備: 常に false を返す述語
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let filtered = fwc.filter_not(|_: &i32| false);

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 10), (2, 20)]).via(filtered.as_flow()).collect_values().unwrap();

  // 検証: 全要素が通過
  assert_eq!(values, vec![(1, 10), (2, 20)]);
}

// --- collect テスト ---

#[test]
fn collect_filters_and_maps_preserving_context() {
  // 準備: 正の値のみ2倍にして収集する FlowWithContext
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let collected = fwc.collect(|v: i32| if v > 0 { Some(v * 2) } else { None });

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 5), (2, -3), (3, 10)]).via(collected.as_flow()).collect_values().unwrap();

  // 検証: Some の結果のみ通過、変換が適用
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn collect_drops_all_when_all_none() {
  // 準備: 常に None を返す collect 関数
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let collected = fwc.collect(|_: i32| -> Option<i32> { None });

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 5), (2, 10)]).via(collected.as_flow()).collect_values().unwrap();

  // 検証: 要素なし
  assert!(values.is_empty());
}

// --- map_async テスト ---

#[test]
fn map_async_transforms_preserving_context() {
  // 準備: 値を2倍にする非同期マップ付き FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let mapped = fwc.map_async(1, |v: u32| async move { v * 2 }).expect("map_async");

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 5_u32), (2, 3)]).via(mapped.as_flow()).collect_values().unwrap();

  // 検証: 値は変換され、コンテキストは保持
  assert_eq!(values, vec![(1, 10_u32), (2, 6)]);
}

// --- grouped テスト ---

#[test]
fn grouped_collects_elements_with_last_context() {
  // 準備: サイズ2でグループ化する FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let grouped = fwc.grouped(2).expect("grouped");

  // 実行: 5要素を流す
  let values = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4), (50, 5)])
    .via(grouped.as_flow())
    .collect_values()
    .unwrap();

  // 検証: 各グループのコンテキストは最後の要素のもの
  assert_eq!(values, vec![(20, vec![1_u32, 2]), (40, vec![3, 4]), (50, vec![5])]);
}

#[test]
fn grouped_single_element_per_group() {
  // 準備: グループサイズ1
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let grouped = fwc.grouped(1).expect("grouped");

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20)]).via(grouped.as_flow()).collect_values().unwrap();

  // 検証: 各要素が独立したグループ、コンテキスト保持
  assert_eq!(values, vec![(1, vec![10_u32]), (2, vec![20])]);
}

// --- sliding テスト ---

#[test]
fn sliding_creates_windows_with_last_context() {
  // 準備: サイズ3のスライディングウィンドウ付き FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let sliding = fwc.sliding(3).expect("sliding");

  // 実行: 要素を流す
  let values =
    Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4)]).via(sliding.as_flow()).collect_values().unwrap();

  // 検証: 各ウィンドウのコンテキストは最後の要素のもの
  assert_eq!(values, vec![(30, vec![1_u32, 2, 3]), (40, vec![2, 3, 4]),]);
}

#[test]
fn sliding_window_size_2() {
  // 準備: サイズ2のスライディングウィンドウ付き FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let sliding = fwc.sliding(2).expect("sliding");

  // 実行: 3要素を流す
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20), (3, 30)]).via(sliding.as_flow()).collect_values().unwrap();

  // 検証: 2つのウィンドウ、各ウィンドウのコンテキストは最後の要素のもの
  assert_eq!(values, vec![(2, vec![10_u32, 20]), (3, vec![20, 30])]);
}
