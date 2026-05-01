use super::SecondarySourceBridge;
use crate::core::{StreamError, dsl::Source, materialization::StreamDone};

#[test]
fn sync_terminal_state_returns_ok_when_completion_pending_and_stream_running() {
  // 起動直後は try_take=None かつ stream が terminal でないため `if` 内に
  // 入らずそのまま Ok が返る。outer None 腕の false 分岐を網羅する。
  let source = Source::from_array([1_u32, 2, 3]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.sync_terminal_state().expect("sync");
  assert!(!bridge.finished);
}

#[test]
fn sync_terminal_state_finalizes_on_completion_ok() {
  // completion を Ok でセットしてから sync_terminal_state を呼ぶ。
  // try_take=Some(Ok) の outer 腕で finished=true になる。
  let source = Source::from_array([1_u32]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.completion.complete(Ok(StreamDone::new()));
  bridge.sync_terminal_state().expect("sync");
  assert!(bridge.finished);
}

#[test]
fn sync_terminal_state_propagates_completion_err() {
  // completion を Err でセットしてから sync_terminal_state を呼ぶ。
  // try_take=Some(Err) の outer 腕でエラーが伝播する。
  let source = Source::from_array([1_u32]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.completion.complete(Err(StreamError::Failed));
  let result = bridge.sync_terminal_state();
  assert_eq!(result, Err(StreamError::Failed));
  assert!(bridge.finished);
}

#[test]
fn sync_terminal_state_finalizes_on_terminal_when_completion_consumed() {
  // stream を terminal まで駆動 → completion を try_take で消費 →
  // sync_terminal_state を呼ぶことで、outer None + is_terminal=true の経路
  // (inner re-poll が `Some(Err)` 以外で抜けるパス) を網羅する。
  let source = Source::from_array([1_u32]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  for _ in 0..64 {
    if bridge.stream.state().is_terminal() {
      break;
    }
    let _ = bridge.stream.drive();
  }
  assert!(bridge.stream.state().is_terminal(), "stream should reach terminal state");
  while bridge.queue.poll().expect("poll").is_some() {}
  let _ = bridge.completion.try_take();
  bridge.sync_terminal_state().expect("sync");
  assert!(bridge.finished);
}
