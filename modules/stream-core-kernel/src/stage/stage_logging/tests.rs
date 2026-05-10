use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::stage::StageLogging;

// --- テスト用モック実装 ---

/// ログ送信を捕捉するモック StageLogging 実装。
/// デフォルト実装の動作を検証するため、メソッド呼び出しを記録する。
struct RecordingLogger {
  name:   String,
  events: ArcShared<SpinSyncMutex<Vec<(String, String)>>>,
}

impl RecordingLogger {
  fn new(name: &str) -> Self {
    Self { name: name.to_string(), events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn record(&self, level: &str, msg: &str) {
    let mut guard = self.events.lock();
    guard.push((level.to_string(), msg.to_string()));
  }

  fn events(&self) -> Vec<(String, String)> {
    let guard = self.events.lock();
    guard.clone()
  }
}

impl StageLogging for RecordingLogger {
  fn log_stage_name(&self) -> &str {
    &self.name
  }

  fn log_trace(&self, msg: &str) {
    self.record("trace", msg);
  }

  fn log_debug(&self, msg: &str) {
    self.record("debug", msg);
  }

  fn log_info(&self, msg: &str) {
    self.record("info", msg);
  }

  fn log_warn(&self, msg: &str) {
    self.record("warn", msg);
  }

  fn log_error(&self, msg: &str) {
    self.record("error", msg);
  }
}

// --- トレイト実装可能性 ---

#[test]
fn trait_can_be_implemented() {
  fn assert_impls<T: StageLogging>() {}
  assert_impls::<RecordingLogger>();
}

#[test]
fn trait_object_is_supported() {
  let logger: Box<dyn StageLogging> = Box::new(RecordingLogger::new("stage-1"));

  // 抽象化経由でメソッド呼び出しが可能であること
  assert_eq!(logger.log_stage_name(), "stage-1");
}

// --- log_stage_name: 識別子取得 ---

#[test]
fn log_stage_name_returns_provided_identifier() {
  let logger = RecordingLogger::new("my-stage");
  assert_eq!(logger.log_stage_name(), "my-stage");
}

#[test]
fn log_stage_name_returns_empty_when_not_set() {
  let logger = RecordingLogger::new("");
  assert_eq!(logger.log_stage_name(), "");
}

// --- 各ログレベルメソッド呼び出し ---

#[test]
fn log_trace_passes_message_through() {
  let logger = RecordingLogger::new("s");
  logger.log_trace("hello");

  let events = logger.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].0, "trace");
  assert_eq!(events[0].1, "hello");
}

#[test]
fn log_debug_passes_message_through() {
  let logger = RecordingLogger::new("s");
  logger.log_debug("debug message");

  let events = logger.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].0, "debug");
  assert_eq!(events[0].1, "debug message");
}

#[test]
fn log_info_passes_message_through() {
  let logger = RecordingLogger::new("s");
  logger.log_info("info");

  let events = logger.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].0, "info");
  assert_eq!(events[0].1, "info");
}

#[test]
fn log_warn_passes_message_through() {
  let logger = RecordingLogger::new("s");
  logger.log_warn("warn");

  let events = logger.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].0, "warn");
  assert_eq!(events[0].1, "warn");
}

#[test]
fn log_error_passes_message_through() {
  let logger = RecordingLogger::new("s");
  logger.log_error("error");

  let events = logger.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].0, "error");
  assert_eq!(events[0].1, "error");
}

#[test]
fn multiple_log_calls_preserve_order() {
  let logger = RecordingLogger::new("s");
  logger.log_info("first");
  logger.log_warn("second");
  logger.log_error("third");

  let events = logger.events();
  assert_eq!(events.len(), 3);
  assert_eq!(events[0].1, "first");
  assert_eq!(events[1].1, "second");
  assert_eq!(events[2].1, "third");
}

// --- &self API（CQS: Query側） ---

#[test]
fn stage_name_is_queryable_without_mutable_reference() {
  let logger = RecordingLogger::new("read-only");

  // &self API のみでアクセス可能（CQS Query 側を担保）
  let name1 = logger.log_stage_name();
  let name2 = logger.log_stage_name();
  assert_eq!(name1, name2);
}
