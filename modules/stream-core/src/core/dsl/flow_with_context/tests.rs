use alloc::{vec, vec::Vec};
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};
use std::{
  path::Path,
  sync::{Arc, Mutex},
};

use crate::core::{
  StreamError, ThrottleMode,
  dsl::{Flow, FlowWithContext, Sink, Source},
  materialization::{KeepBoth, StreamNotUsed},
};

#[derive(Default)]
struct YieldThenOutputFuture<T> {
  value:       Option<T>,
  poll_count:  u8,
  ready_after: u8,
}

impl<T> YieldThenOutputFuture<T> {
  fn new(value: T, ready_after: u8) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after }
  }
}

impl<T: Unpin> Future for YieldThenOutputFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      cx.waker().wake_by_ref();
      Poll::Pending
    } else {
      Poll::Ready(this.value.take().expect("future value"))
    }
  }
}

#[test]
fn should_map_output_preserving_context() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v)).map(|s: &str| s.len());
  let values = Source::from(vec![(1_i32, "hello"), (2, "world")]).via(fwc.into_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v)).filter(|v: &i32| *v > 0);
  let values = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]).via(fwc.into_flow()).collect_values().unwrap();
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
  let values = Source::from(vec![(10_i64, "a"), (20_i64, "b")]).via(mapped.into_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(90_i64, "a"), (190_i64, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc1: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let fwc2: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let composed = fwc1.via(fwc2);
  let values = Source::from(vec![(1_i32, "hello"), (2, "hi")]).via(composed.into_flow()).collect_values().unwrap();
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
  let values = Source::from(vec![(1_i32, "ab"), (2, "c")]).via(expanded.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 5), (2, -1), (3, 3)]).via(expanded.into_flow()).collect_values().unwrap();

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
    Source::from(vec![(1_i32, 10), (2, -5), (3, 0), (4, 20)]).via(filtered.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 10), (2, 20)]).via(filtered.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 5), (2, -3), (3, 10)]).via(collected.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 5), (2, 10)]).via(collected.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 5_u32), (2, 3)]).via(mapped.into_flow()).collect_values().unwrap();

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
    .via(grouped.into_flow())
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
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20)]).via(grouped.into_flow()).collect_values().unwrap();

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
    Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4)]).via(sliding.into_flow()).collect_values().unwrap();

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
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20), (3, 30)]).via(sliding.into_flow()).collect_values().unwrap();

  // 検証: 2つのウィンドウ、各ウィンドウのコンテキストは最後の要素のもの
  assert_eq!(values, vec![(2, vec![10_u32, 20]), (3, vec![20, 30])]);
}

#[test]
fn via_mat_combines_materialized_values() {
  // 準備: 2つの FlowWithContext がそれぞれ異なるマテリアライズド値を持つ
  let left: FlowWithContext<i32, u32, u32, u32> =
    FlowWithContext::from_flow(Flow::new().map(|value: (i32, u32)| value).map_materialized_value(|_| 7_u32));
  let right: FlowWithContext<i32, u32, u32, u32> = FlowWithContext::from_flow(
    Flow::new().map(|(ctx, value): (i32, u32)| (ctx, value + 1)).map_materialized_value(|_| 9_u32),
  );

  // 実行: KeepBoth で 2つの flow を合成する
  let (_graph, materialized) = left.via_mat(right, KeepBoth).into_flow().into_parts();

  // 検証: 両方のマテリアライズド値が保持される
  assert_eq!(materialized, (7_u32, 9_u32));
}

#[test]
fn also_to_keeps_main_path_values_and_context() {
  // 準備: side sink を接続した context 保持 flow
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|value: (i32, u32)| value)).also_to(Sink::<u32, _>::ignore());

  // 実行: 要素を flow に流す
  let values = Source::from(vec![(10_i32, 1_u32), (20, 2)]).via(flow.into_flow()).collect_values().unwrap();

  // 検証: main path の値とコンテキストは変化しない
  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
}

#[test]
fn also_to_sends_values_to_side_sink_and_preserves_main_path() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|value: (i32, u32)| value)).also_to(Sink::foreach(move |value: u32| {
      seen_for_sink.lock().expect("side sink lock").push(value);
    }));

  let values = Source::from(vec![(10_i32, 1_u32), (20, 2)]).via(flow.into_flow()).collect_values().unwrap();

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![1_u32, 2]);
}

#[test]
fn also_to_context_sends_only_contexts_to_side_sink() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> = FlowWithContext::from_flow(
    Flow::new().map(|value: (i32, u32)| value),
  )
  .also_to_context(Sink::foreach(move |ctx: i32| {
    seen_for_sink.lock().expect("context sink lock").push(ctx);
  }));

  let values = Source::from(vec![(10_i32, 1_u32), (20, 2)]).via(flow.into_flow()).collect_values().unwrap();

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![10_i32, 20]);
}

#[test]
fn wire_tap_preserves_main_path_and_emits_values() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> = FlowWithContext::from_flow(
    Flow::new().map(|value: (i32, u32)| value),
  )
  .wire_tap(Sink::foreach(move |value: u32| {
    seen_for_sink.lock().expect("tap sink lock").push(value);
  }));

  let values = Source::from(vec![(10_i32, 1_u32), (20, 2)]).via(flow.into_flow()).collect_values().unwrap();

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![1_u32, 2]);
}

#[test]
fn wire_tap_context_preserves_main_path_and_emits_contexts() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> = FlowWithContext::from_flow(
    Flow::new().map(|value: (i32, u32)| value),
  )
  .wire_tap_context(Sink::foreach(move |ctx: i32| {
    seen_for_sink.lock().expect("context tap lock").push(ctx);
  }));

  let values = Source::from(vec![(10_i32, 1_u32), (20, 2)]).via(flow.into_flow()).collect_values().unwrap();

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![10_i32, 20]);
}

#[test]
fn map_async_partitioned_preserves_context_and_input_order() {
  // 準備: 後続 partition の方が先に完了しうる非同期マップ
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|value: (i32, u32)| value));
  let mapped = flow
    .map_async_partitioned(
      2,
      |value: &u32| (*value as usize) % 2,
      |value: u32, partition: usize| {
        let ready_after = if partition == 1 { 2 } else { 0 };
        YieldThenOutputFuture::new(value + 10, ready_after)
      },
    )
    .expect("map_async_partitioned");

  // 実行: source 経由で materialize して要素を収集する
  let values = Source::from(vec![(100_i32, 1_u32), (200, 2)]).via(mapped.into_flow()).collect_values().unwrap();

  // 検証: 入力順が保たれ、各要素は元のコンテキストを保持する
  assert_eq!(values, vec![(100_i32, 11_u32), (200, 12)]);
}

#[test]
fn map_async_partitioned_unordered_can_emit_completion_order_while_preserving_context() {
  // 準備: 先頭入力の完了が後ろにずれる非同期マップ
  let flow: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|value: (i32, u32)| value));
  let mapped = flow
    .map_async_partitioned_unordered(
      2,
      |value: &u32| (*value as usize) % 2,
      |value: u32, partition: usize| {
        let ready_after = if partition == 1 { 16 } else { 0 };
        YieldThenOutputFuture::new(value + 10, ready_after)
      },
    )
    .expect("map_async_partitioned_unordered");

  // 実行: source 経由で materialize して要素を収集する
  let values = Source::from(vec![(100_i32, 1_u32), (200, 2)]).via(mapped.into_flow()).collect_values().unwrap();

  // 検証: 完了順は入れ替わりうるが、値とコンテキストの対応は維持される
  assert_eq!(values, vec![(200_i32, 12_u32), (100, 11)]);
}

// --- map_error テスト ---

#[test]
fn map_error_passes_normal_elements_preserving_context() {
  // 準備: map_error を適用した FlowWithContext（通常要素はそのまま通過する）
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let mapped = fwc.map_error(|_| StreamError::WouldBlock);

  // 実行: 正常な要素を流す
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20)]).via(mapped.into_flow()).collect_values().unwrap();

  // 検証: 正常な要素はコンテキスト付きでそのまま通過する
  assert_eq!(values, vec![(1, 10_u32), (2, 20)]);
}

#[test]
fn map_error_transforms_upstream_failure_preserving_context_flow() {
  // 準備: 失敗する source に map_error を適用した FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let mapped = fwc.map_error(|_| StreamError::WouldBlock);

  // 実行: 失敗する source を flow に接続
  let result = Source::<(i32, u32), _>::failed(StreamError::Failed).via(mapped.into_flow()).collect_values();

  // 検証: エラーが変換される
  assert_eq!(result, Err(StreamError::WouldBlock));
}

// --- throttle テスト ---

#[test]
fn throttle_passes_elements_preserving_context() {
  // 準備: Shaping モードの throttle を適用した FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let throttled = fwc.throttle(2, ThrottleMode::Shaping).expect("throttle");

  // 実行: 要素を流す
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20)]).via(throttled.into_flow()).collect_values().unwrap();

  // 検証: 要素はコンテキスト付きでそのまま通過する
  assert_eq!(values, vec![(1, 10_u32), (2, 20)]);
}

#[test]
fn throttle_enforcing_mode_preserves_context() {
  // 準備: Enforcing モードの throttle を適用した FlowWithContext
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let throttled = fwc.throttle(2, ThrottleMode::Enforcing).expect("throttle");

  // 実行: 単一要素を流す（キャパシティ内）
  let values = Source::from(vec![(1_i32, 10_u32)]).via(throttled.into_flow()).collect_values().unwrap();

  // 検証: 要素はコンテキスト付きでそのまま通過する
  assert_eq!(values, vec![(1, 10_u32)]);
}

#[test]
fn throttle_rejects_zero_capacity_on_context_flow() {
  // 準備: ゼロキャパシティの throttle
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));

  // 実行: ゼロキャパシティで throttle を作成
  let result = fwc.throttle(0, ThrottleMode::Shaping);

  // 検証: InvalidArgument エラーが返る
  assert!(result.is_err());
}

#[test]
fn comments_use_japanese_in_context_wrapper_tests() {
  let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/dsl");
  let files = [
    ("flow_with_context/tests.rs", base.join("flow_with_context/tests.rs")),
    ("source_with_context/tests.rs", base.join("source_with_context/tests.rs")),
  ];
  let forbidden_markers =
    [concat!("// ", "Given", ":"), concat!("// ", "When", ":"), concat!("// ", "Then", ":"), concat!("tests", " ---")];

  for (label, path) in files {
    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{label} の読み込みに失敗: {e}"));
    for marker in forbidden_markers {
      assert!(!content.contains(marker), "{label} に英語コメントの残骸 `{marker}` が残っています");
    }
  }
}
