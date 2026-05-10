use super::FlowMonitor;
use crate::{
  StreamError,
  dsl::{FlowMonitorImpl, FlowMonitorState},
};

// --- FlowMonitorImpl: 初期状態 ---

#[test]
fn new_monitor_starts_in_initialized_state() {
  // 準備: 新しく作成したフローモニター
  let monitor = FlowMonitorImpl::<u32>::new();

  // 実行: 現在の状態を問い合わせ
  let state = monitor.state();

  // 検証: 状態は Initialized
  assert!(matches!(state, FlowMonitorState::Initialized));
}

// --- 状態遷移 ---

#[test]
fn monitor_transitions_to_received_on_element() {
  // 準備: フローモニター
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // 実行: 受信した要素で状態を更新
  monitor.set_state(FlowMonitorState::Received(42));

  // 検証: 受信値が反映される
  assert!(matches!(monitor.state(), FlowMonitorState::Received(42)));
}

#[test]
fn monitor_transitions_to_failed_on_error() {
  // 準備: フローモニター
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // 実行: 失敗状態を設定
  monitor.set_state(FlowMonitorState::Failed(StreamError::Failed));

  // 検証: 失敗が反映される
  assert!(matches!(monitor.state(), FlowMonitorState::Failed(StreamError::Failed)));
}

#[test]
fn monitor_transitions_to_finished_on_completion() {
  // 準備: フローモニター
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // 実行: 完了状態を設定
  monitor.set_state(FlowMonitorState::Finished);

  // 検証: 完了が反映される
  assert!(matches!(monitor.state(), FlowMonitorState::Finished));
}

#[test]
fn monitor_overwrites_previous_received_value() {
  // 準備: 値を受信済みのモニター
  let mut monitor = FlowMonitorImpl::<u32>::new();
  monitor.set_state(FlowMonitorState::Received(1));

  // 実行: 新しい値を受信
  monitor.set_state(FlowMonitorState::Received(2));

  // 検証: 最新の値のみ反映
  assert!(matches!(monitor.state(), FlowMonitorState::Received(2)));
}

// --- FlowMonitor トレイト経由の使用 ---

#[test]
fn flow_monitor_trait_state_returns_correct_state() {
  // 準備: FlowMonitor トレイト経由のモニター
  let monitor = FlowMonitorImpl::<u32>::new();
  let trait_ref: &dyn FlowMonitor<u32> = &monitor;

  // 実行: トレイト経由で state() を呼び出し
  let state = trait_ref.state();

  // 検証: 正しい初期状態が返される
  assert!(matches!(state, FlowMonitorState::Initialized));
}
