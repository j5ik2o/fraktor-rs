use alloc::{
  borrow::Cow,
  boxed::Box,
  string::{String, ToString},
  vec,
  vec::Vec,
};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{
  attributes::{LogLevel, SourceLocation},
  materialization::MaterializerLoggingProvider,
};

// --- テスト用モック実装 ---

/// 呼び出された (level, message, location) を記録するモック。
struct RecordingLoggingProvider {
  enabled_from: LogLevel,
  records:      ArcShared<SpinSyncMutex<Vec<(LogLevel, String, Option<SourceLocation>)>>>,
}

impl RecordingLoggingProvider {
  fn new(enabled_from: LogLevel) -> Self {
    Self { enabled_from, records: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn records(&self) -> Vec<(LogLevel, String, Option<SourceLocation>)> {
    let guard = self.records.lock();
    guard.clone()
  }

  fn level_priority(level: LogLevel) -> u8 {
    match level {
      | LogLevel::Off => 0,
      | LogLevel::Error => 1,
      | LogLevel::Warning => 2,
      | LogLevel::Info => 3,
      | LogLevel::Debug => 4,
    }
  }
}

impl MaterializerLoggingProvider for RecordingLoggingProvider {
  fn is_enabled(&self, level: LogLevel) -> bool {
    // enabled_from よりも優先度が同等以下（Error <= enabled_from 等）なら enabled
    Self::level_priority(level) <= Self::level_priority(self.enabled_from) && level != LogLevel::Off
  }

  fn log(&self, level: LogLevel, message: &str, source_location: Option<&SourceLocation>) {
    let mut guard = self.records.lock();
    guard.push((level, message.to_string(), source_location.cloned()));
  }
}

fn fixture_location() -> SourceLocation {
  SourceLocation::new(Cow::Borrowed("src/lib.rs"), 42, 7)
}

// --- トレイト trait 実装可能性 ---

#[test]
fn trait_can_be_implemented() {
  fn assert_impls<T: MaterializerLoggingProvider>() {}
  assert_impls::<RecordingLoggingProvider>();
}

#[test]
fn trait_object_is_supported() {
  // dyn MaterializerLoggingProvider が成立すること（plan で明記された前提）
  let provider: Box<dyn MaterializerLoggingProvider> = Box::new(RecordingLoggingProvider::new(LogLevel::Info));

  // trait object 経由で is_enabled 呼び出し可能であること
  assert!(provider.is_enabled(LogLevel::Error));
}

// --- is_enabled 契約 ---

#[test]
fn is_enabled_returns_true_for_levels_at_or_below_configured() {
  let provider = RecordingLoggingProvider::new(LogLevel::Info);

  // Error (1) <= Info (3) → enabled
  assert!(provider.is_enabled(LogLevel::Error));
  assert!(provider.is_enabled(LogLevel::Warning));
  assert!(provider.is_enabled(LogLevel::Info));
}

#[test]
fn is_enabled_returns_false_for_levels_above_configured() {
  let provider = RecordingLoggingProvider::new(LogLevel::Warning);

  // Info (3) > Warning (2) → disabled
  assert!(!provider.is_enabled(LogLevel::Info));
  assert!(!provider.is_enabled(LogLevel::Debug));
}

#[test]
fn is_enabled_returns_false_for_off_level() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);

  // Off は常に false（有効化不可）
  assert!(!provider.is_enabled(LogLevel::Off));
}

#[test]
fn is_enabled_is_query_style() {
  // CQS Query: &self で呼べること（=内部可変性なしで問い合わせ可能）
  let provider = RecordingLoggingProvider::new(LogLevel::Info);
  let first = provider.is_enabled(LogLevel::Error);
  let second = provider.is_enabled(LogLevel::Error);
  // 同じ呼び出しで同じ値が返ること（副作用なし）
  assert_eq!(first, second);
}

// --- log 契約 ---

#[test]
fn log_records_level_and_message() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);

  provider.log(LogLevel::Info, "hello", None);

  let records = provider.records();
  assert_eq!(records.len(), 1);
  assert_eq!(records[0].0, LogLevel::Info);
  assert_eq!(records[0].1, "hello");
  assert!(records[0].2.is_none());
}

#[test]
fn log_records_source_location_when_provided() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);
  let location = fixture_location();

  provider.log(LogLevel::Warning, "warn msg", Some(&location));

  let records = provider.records();
  assert_eq!(records.len(), 1);
  assert_eq!(records[0].0, LogLevel::Warning);
  assert_eq!(records[0].1, "warn msg");
  assert_eq!(records[0].2.as_ref(), Some(&location));
}

#[test]
fn log_accepts_each_level_variant() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);

  // Off を除く全レベルで log を呼べることを確認
  provider.log(LogLevel::Error, "e", None);
  provider.log(LogLevel::Warning, "w", None);
  provider.log(LogLevel::Info, "i", None);
  provider.log(LogLevel::Debug, "d", None);

  let records = provider.records();
  assert_eq!(records.len(), 4);
  assert_eq!(records.iter().map(|(lvl, _, _)| *lvl).collect::<Vec<_>>(), vec![
    LogLevel::Error,
    LogLevel::Warning,
    LogLevel::Info,
    LogLevel::Debug
  ],);
}

#[test]
fn log_preserves_call_order() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);

  provider.log(LogLevel::Info, "first", None);
  provider.log(LogLevel::Info, "second", None);
  provider.log(LogLevel::Info, "third", None);

  let records = provider.records();
  assert_eq!(records.len(), 3);
  assert_eq!(records[0].1, "first");
  assert_eq!(records[1].1, "second");
  assert_eq!(records[2].1, "third");
}

#[test]
fn log_is_callable_via_trait_object() {
  // dyn 越しに log 呼び出しが可能であること
  let provider: Box<dyn MaterializerLoggingProvider> = Box::new(RecordingLoggingProvider::new(LogLevel::Debug));

  provider.log(LogLevel::Error, "trait object log", None);
}

#[test]
fn log_accepts_empty_message() {
  let provider = RecordingLoggingProvider::new(LogLevel::Debug);

  // 空メッセージでもパニックせず記録されること
  provider.log(LogLevel::Info, "", None);

  let records = provider.records();
  assert_eq!(records.len(), 1);
  assert_eq!(records[0].1, "");
}
