use alloc::vec;

use crate::snapshot::{RunningInterpreter, StreamSnapshot, UninitializedInterpreter};

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_snapshot_with_empty_vectors_is_empty() {
  // Given: active_interpreters と new_shells がどちらも空
  let snapshot = StreamSnapshot::new(vec![], vec![]);

  // Then: 両 accessor が空スライスを返す
  assert!(snapshot.active_interpreters().is_empty());
  assert!(snapshot.new_shells().is_empty());
}

#[test]
fn new_snapshot_preserves_active_interpreters() {
  // Given: 1 件の RunningInterpreter を active_interpreters として構築する
  let interp = RunningInterpreter::new(vec![], vec![], 0, vec![]);
  let snapshot = StreamSnapshot::new(vec![interp], vec![]);

  // Then: active_interpreters には 1 件だけ格納されている
  assert_eq!(snapshot.active_interpreters().len(), 1);
  assert!(snapshot.new_shells().is_empty());
}

#[test]
fn new_snapshot_preserves_new_shells() {
  // Given: 1 件の UninitializedInterpreter を new_shells として構築する
  let shell = UninitializedInterpreter::new(vec![]);
  let snapshot = StreamSnapshot::new(vec![], vec![shell]);

  // Then: new_shells には 1 件だけ格納されている
  assert!(snapshot.active_interpreters().is_empty());
  assert_eq!(snapshot.new_shells().len(), 1);
}

#[test]
fn new_snapshot_with_multiple_entries_in_both_vectors() {
  // Given: 2 件の RunningInterpreter と 3 件の UninitializedInterpreter
  let actives =
    vec![RunningInterpreter::new(vec![], vec![], 0, vec![]), RunningInterpreter::new(vec![], vec![], 0, vec![])];
  let shells = vec![
    UninitializedInterpreter::new(vec![]),
    UninitializedInterpreter::new(vec![]),
    UninitializedInterpreter::new(vec![]),
  ];

  let snapshot = StreamSnapshot::new(actives, shells);

  // Then: 両 accessor が与えた長さを反映する
  assert_eq!(snapshot.active_interpreters().len(), 2);
  assert_eq!(snapshot.new_shells().len(), 3);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_produces_independent_snapshot_with_same_shape() {
  // Given: active / new_shells に 1 件ずつ格納された snapshot
  let original =
    StreamSnapshot::new(vec![RunningInterpreter::new(vec![], vec![], 0, vec![])], vec![UninitializedInterpreter::new(
      vec![],
    )]);

  // When: クローンする
  let cloned = original.clone();

  // Then: クローン側が同じ形状を持つ
  assert_eq!(cloned.active_interpreters().len(), 1);
  assert_eq!(cloned.new_shells().len(), 1);
  // And: オリジナルは移動されておらず依然として有効
  assert_eq!(original.active_interpreters().len(), 1);
}

#[test]
fn debug_format_is_non_empty() {
  // Given: 空の StreamSnapshot
  let snapshot = StreamSnapshot::new(vec![], vec![]);

  // When: Debug フォーマットする
  let debug = alloc::format!("{snapshot:?}");

  // Then: 型名が出力に含まれる
  assert!(debug.contains("StreamSnapshot"));
}
