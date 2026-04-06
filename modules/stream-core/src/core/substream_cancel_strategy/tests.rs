use crate::core::SubstreamCancelStrategy;

#[test]
fn default_returns_propagate() {
  // 前提/操作: デフォルトの SubstreamCancelStrategy を取得する
  let strategy = SubstreamCancelStrategy::default();

  // 期待: 既定値は Pekko と同じく Propagate である
  assert_eq!(strategy, SubstreamCancelStrategy::Propagate);
}

#[test]
fn drain_variant_is_distinct_from_propagate() {
  let drain = SubstreamCancelStrategy::Drain;
  let propagate = SubstreamCancelStrategy::Propagate;

  assert_ne!(drain, propagate);
}

#[test]
fn clone_preserves_variant() {
  let drain = SubstreamCancelStrategy::Drain;
  let cloned = drain.clone();

  assert_eq!(drain, cloned);
}

#[test]
fn debug_format_includes_variant_name() {
  let drain = SubstreamCancelStrategy::Drain;
  let propagate = SubstreamCancelStrategy::Propagate;

  let drain_debug = alloc::format!("{:?}", drain);
  let propagate_debug = alloc::format!("{:?}", propagate);

  assert!(drain_debug.contains("Drain"));
  assert!(propagate_debug.contains("Propagate"));
}
