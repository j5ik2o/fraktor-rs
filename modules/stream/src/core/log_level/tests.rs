use crate::core::LogLevel;

// --- variant construction ---

#[test]
fn all_variants_are_constructible() {
  let _ = LogLevel::Off;
  let _ = LogLevel::Error;
  let _ = LogLevel::Warning;
  let _ = LogLevel::Info;
  let _ = LogLevel::Debug;
}

// --- equality ---

#[test]
fn same_variants_are_equal() {
  assert_eq!(LogLevel::Off, LogLevel::Off);
  assert_eq!(LogLevel::Error, LogLevel::Error);
  assert_eq!(LogLevel::Warning, LogLevel::Warning);
  assert_eq!(LogLevel::Info, LogLevel::Info);
  assert_eq!(LogLevel::Debug, LogLevel::Debug);
}

#[test]
fn different_variants_are_not_equal() {
  assert_ne!(LogLevel::Off, LogLevel::Debug);
  assert_ne!(LogLevel::Error, LogLevel::Warning);
  assert_ne!(LogLevel::Info, LogLevel::Off);
}

// --- clone ---

#[test]
fn clone_preserves_variant() {
  let original = LogLevel::Warning;
  let cloned = original.clone();
  assert_eq!(original, cloned);
}

// --- copy ---

#[test]
fn copy_semantics_work() {
  let a = LogLevel::Info;
  let b = a;
  // Both are usable after copy
  assert_eq!(a, b);
}

// --- debug ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", LogLevel::Error);
  assert!(!debug.is_empty());
}
