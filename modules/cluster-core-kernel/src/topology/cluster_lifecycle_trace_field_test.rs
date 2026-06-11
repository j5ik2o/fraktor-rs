// タスク 1.2: 遷移種別 const 値の一意性テスト（要件 4.1）
// 遷移種別ごとに一意な値を持つことを exhaustive に検証する

use crate::topology::cluster_lifecycle_trace_field::{
  FIELD_AUTHORITY, FIELD_DATA_CENTER, FIELD_NODE_ID, FIELD_TRANSITION, TRANSITION_DC_REACHABLE,
  TRANSITION_DC_UNREACHABLE, TRANSITION_JOIN, TRANSITION_LEAVE, TRANSITION_REMOVAL, TRANSITION_SHUTDOWN_PREPARING,
  TRANSITION_SHUTDOWN_READY, TRANSITION_UP,
};

#[test]
fn transition_kind_values_are_unique() {
  // 遷移種別の const 値を収集して重複がないことを確認する
  let values = [
    TRANSITION_JOIN,
    TRANSITION_UP,
    TRANSITION_LEAVE,
    TRANSITION_REMOVAL,
    TRANSITION_SHUTDOWN_PREPARING,
    TRANSITION_SHUTDOWN_READY,
    TRANSITION_DC_UNREACHABLE,
    TRANSITION_DC_REACHABLE,
  ];
  // 重複がないことを確認する
  for i in 0..values.len() {
    for j in (i + 1)..values.len() {
      assert_ne!(values[i], values[j], "遷移種別の値が重複しています: index {} と {} がともに {:?}", i, j, values[i]);
    }
  }
}

#[test]
fn field_name_constants_are_defined() {
  // member 識別・data center・遷移種別のフィールド名が空でないことを確認する（要件 4.2）
  assert!(!FIELD_NODE_ID.is_empty());
  assert!(!FIELD_AUTHORITY.is_empty());
  assert!(!FIELD_DATA_CENTER.is_empty());
  assert!(!FIELD_TRANSITION.is_empty());
}
