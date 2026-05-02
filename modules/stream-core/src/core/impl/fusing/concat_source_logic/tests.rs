use super::SecondarySourceBridge;
use crate::core::{StreamError, dsl::Source, materialization::StreamDone, stage::CancellationCause};

#[test]
fn sync_terminal_state_returns_ok_when_completion_pending_and_stream_running() {
  // 起動直後は try_take=None かつ stream 状態が Idle/Running のため、
  // outer None 腕の Idle/Running 分岐 (`Ok(())`) を網羅する。
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
fn sync_terminal_state_finalizes_when_stream_completed_and_completion_consumed() {
  // stream を Completed まで駆動 → completion を try_take で消費 →
  // sync_terminal_state を呼ぶことで outer None + StreamState::Completed
  // 腕 (`complete_if_open`) を網羅する。
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

#[test]
fn sync_terminal_state_propagates_failed_when_stream_failed_and_completion_absent() {
  // stream を abort で Failed 状態にし completion 未設定で sync_terminal_state
  // を呼ぶ。outer None + StreamState::Failed 腕で StreamError::Failed が
  // 伝播する。
  let source = Source::from_array([1_u32]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.stream.abort(&StreamError::Failed);
  let _ = bridge.completion.try_take();
  let result = bridge.sync_terminal_state();
  assert_eq!(result, Err(StreamError::Failed));
  assert!(bridge.finished);
}

#[test]
fn sync_terminal_state_propagates_cancellation_when_stream_cancelled_and_completion_absent() {
  // stream を cancel で Cancelled 状態にし completion 未設定で
  // sync_terminal_state を呼ぶ。outer None + StreamState::Cancelled 腕で
  // CancellationCause が伝播する。
  let source = Source::from_array([1_u32]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.stream.cancel().expect("cancel");
  let _ = bridge.completion.try_take();
  let result = bridge.sync_terminal_state();
  assert_eq!(result, Err(StreamError::CancellationCause { cause: CancellationCause::stage_was_completed() }));
  assert!(bridge.finished);
}
