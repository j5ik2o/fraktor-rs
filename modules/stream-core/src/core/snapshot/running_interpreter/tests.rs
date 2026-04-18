use alloc::{boxed::Box, vec};

use crate::core::{
  attributes::Attributes,
  snapshot::{ConnectionSnapshot, ConnectionState, InterpreterSnapshot, LogicSnapshot, RunningInterpreter},
};

// ローカルヘルパー: 指定の index / label で LogicSnapshot を生成する。
fn logic(index: u32, label: &'static str) -> LogicSnapshot {
  LogicSnapshot::new(index, label, Attributes::new())
}

// ローカルヘルパー: 指定の id / 上流・下流 index で ConnectionSnapshot を生成する。
fn connection(id: u32, up: u32, down: u32) -> ConnectionSnapshot {
  ConnectionSnapshot::new(id, logic(up, "upstream"), logic(down, "downstream"), ConnectionState::ShouldPull)
}

// ---------------------------------------------------------------------------
// Construction: 空コレクション
// ---------------------------------------------------------------------------

#[test]
fn new_with_empty_collections_returns_zero_length_accessors() {
  // Given: logics / connections / stopped_logics がすべて空、running_logics_count = 0
  let interp = RunningInterpreter::new(vec![], vec![], 0, vec![]);

  // Then: 4 accessor がすべて空・ゼロを返す
  assert!(interp.logics().is_empty());
  assert!(interp.connections().is_empty());
  assert_eq!(interp.running_logics_count(), 0);
  assert!(interp.stopped_logics().is_empty());
}

// ---------------------------------------------------------------------------
// Construction: 複数要素
// ---------------------------------------------------------------------------

#[test]
fn new_preserves_logics_in_order() {
  // Given: 3 件の LogicSnapshot
  let l0 = logic(0, "first");
  let l1 = logic(1, "second");
  let l2 = logic(2, "third");

  // When: RunningInterpreter に格納する
  let interp = RunningInterpreter::new(vec![l0, l1, l2], vec![], 3, vec![]);

  // Then: logics() は挿入順で返却する
  let logics = interp.logics();
  assert_eq!(logics.len(), 3);
  assert_eq!(logics[0].index(), 0);
  assert_eq!(logics[1].index(), 1);
  assert_eq!(logics[2].index(), 2);
}

#[test]
fn new_preserves_connections_in_order() {
  // Given: 2 件の ConnectionSnapshot（id=10, 20）
  let c0 = connection(10, 0, 1);
  let c1 = connection(20, 1, 2);

  // When: RunningInterpreter に格納する
  let interp = RunningInterpreter::new(vec![], vec![c0, c1], 0, vec![]);

  // Then: connections() は挿入順で返却する
  let connections = interp.connections();
  assert_eq!(connections.len(), 2);
  assert_eq!(connections[0].id(), 10);
  assert_eq!(connections[1].id(), 20);
}

#[test]
fn new_preserves_running_logics_count() {
  // Given: running_logics_count = 7
  let interp = RunningInterpreter::new(vec![], vec![], 7, vec![]);

  // Then: running_logics_count() は 7 を返す
  assert_eq!(interp.running_logics_count(), 7);
}

#[test]
fn new_preserves_stopped_logics_in_order() {
  // Given: 2 件の停止済み LogicSnapshot
  let s0 = logic(5, "stopped-a");
  let s1 = logic(6, "stopped-b");

  // When: RunningInterpreter に格納する
  let interp = RunningInterpreter::new(vec![], vec![], 0, vec![s0, s1]);

  // Then: stopped_logics() は挿入順で返却する
  let stopped = interp.stopped_logics();
  assert_eq!(stopped.len(), 2);
  assert_eq!(stopped[0].index(), 5);
  assert_eq!(stopped[1].index(), 6);
}

#[test]
fn new_with_all_populated_collections_exposes_all_accessors() {
  // Given: 全フィールドに値が入った RunningInterpreter
  let interp =
    RunningInterpreter::new(vec![logic(0, "l0"), logic(1, "l1")], vec![connection(42, 0, 1)], 2, vec![logic(
      9, "gone",
    )]);

  // Then: 4 accessor すべてが対応する値を返す
  assert_eq!(interp.logics().len(), 2);
  assert_eq!(interp.logics()[0].index(), 0);
  assert_eq!(interp.logics()[1].index(), 1);
  assert_eq!(interp.connections().len(), 1);
  assert_eq!(interp.connections()[0].id(), 42);
  assert_eq!(interp.running_logics_count(), 2);
  assert_eq!(interp.stopped_logics().len(), 1);
  assert_eq!(interp.stopped_logics()[0].index(), 9);
}

// ---------------------------------------------------------------------------
// Boundary values
// ---------------------------------------------------------------------------

#[test]
fn new_accepts_u32_max_running_logics_count() {
  // Given: running_logics_count = u32::MAX
  let interp = RunningInterpreter::new(vec![], vec![], u32::MAX, vec![]);

  // Then: running_logics_count() は u32::MAX を返す
  assert_eq!(interp.running_logics_count(), u32::MAX);
}

// ---------------------------------------------------------------------------
// InterpreterSnapshot trait contract
// ---------------------------------------------------------------------------

#[test]
fn interpreter_snapshot_trait_returns_logics_slice() {
  // Given: 1 件の LogicSnapshot を保持する RunningInterpreter
  let interp = RunningInterpreter::new(vec![logic(42, "only")], vec![], 1, vec![]);

  // When: InterpreterSnapshot trait 経由で要素数を取得する
  fn count<I: InterpreterSnapshot>(i: &I) -> usize {
    i.logics().len()
  }

  // Then: trait 委譲経由でも正しい長さが返る
  assert_eq!(count(&interp), 1);
}

#[test]
fn interpreter_snapshot_is_object_safe_for_running_interpreter() {
  // Given: Box<dyn InterpreterSnapshot> として RunningInterpreter を格納する
  let interp: Box<dyn InterpreterSnapshot> =
    Box::new(RunningInterpreter::new(vec![logic(7, "boxed")], vec![], 1, vec![]));

  // Then: trait object 経由で logics() が委譲される
  assert_eq!(interp.logics().len(), 1);
  assert_eq!(interp.logics()[0].index(), 7);
}

#[test]
fn interpreter_snapshot_trait_exposes_only_active_logics_not_stopped_or_connections() {
  // Given: active logics / connections / stopped_logics がそれぞれ埋まっている RunningInterpreter
  let interp: Box<dyn InterpreterSnapshot> = Box::new(RunningInterpreter::new(
    vec![logic(0, "active-a"), logic(1, "active-b")],
    vec![connection(99, 0, 1)],
    2,
    vec![logic(5, "stopped")],
  ));

  // When: trait 経由で logics() を取得する
  let logics = interp.logics();

  // Then: active logics のみが返り、stopped_logics や connections は混入しない
  assert_eq!(logics.len(), 2);
  assert_eq!(logics[0].index(), 0);
  assert_eq!(logics[1].index(), 1);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_produces_independent_snapshot_with_same_shape() {
  // Given: 全フィールドに値が入った RunningInterpreter
  let original = RunningInterpreter::new(vec![logic(1, "l")], vec![connection(3, 0, 1)], 4, vec![logic(5, "s")]);

  // When: クローンする
  let cloned = original.clone();

  // Then: クローン側が元と同じ形状を持つ
  assert_eq!(cloned.logics().len(), 1);
  assert_eq!(cloned.logics()[0].index(), 1);
  assert_eq!(cloned.connections().len(), 1);
  assert_eq!(cloned.connections()[0].id(), 3);
  assert_eq!(cloned.running_logics_count(), 4);
  assert_eq!(cloned.stopped_logics().len(), 1);
  assert_eq!(cloned.stopped_logics()[0].index(), 5);

  // And: オリジナルは移動されておらず依然として有効
  assert_eq!(original.logics().len(), 1);
  assert_eq!(original.connections().len(), 1);
  assert_eq!(original.running_logics_count(), 4);
  assert_eq!(original.stopped_logics().len(), 1);
}

#[test]
fn debug_format_identifies_type() {
  // Given: 空の RunningInterpreter
  let interp = RunningInterpreter::new(vec![], vec![], 0, vec![]);

  // When: Debug フォーマットする
  let debug = alloc::format!("{interp:?}");

  // Then: 型名が出力に含まれる
  assert!(debug.contains("RunningInterpreter"));
}
