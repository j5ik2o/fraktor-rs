use alloc::{boxed::Box, vec};

use crate::{
  attributes::Attributes,
  snapshot::{InterpreterSnapshot, LogicSnapshot, UninitializedInterpreter},
};

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_with_empty_logics_is_empty() {
  // Given: an interpreter with no logic snapshots
  let interp = UninitializedInterpreter::new(vec![]);

  // Then: logics() returns an empty slice via the trait method
  assert!(interp.logics().is_empty());
}

#[test]
fn new_preserves_logic_snapshots_in_order() {
  // Given: three logic snapshots with distinct indices
  let l0 = LogicSnapshot::new(0, "first", Attributes::new());
  let l1 = LogicSnapshot::new(1, "second", Attributes::new());
  let l2 = LogicSnapshot::new(2, "third", Attributes::new());

  // When: wrapped in an UninitializedInterpreter
  let interp = UninitializedInterpreter::new(vec![l0, l1, l2]);

  // Then: trait-accessor returns them in insertion order
  let logics = interp.logics();
  assert_eq!(logics.len(), 3);
  assert_eq!(logics[0].index(), 0);
  assert_eq!(logics[1].index(), 1);
  assert_eq!(logics[2].index(), 2);
}

// ---------------------------------------------------------------------------
// InterpreterSnapshot trait contract
// ---------------------------------------------------------------------------

#[test]
fn interpreter_snapshot_trait_returns_logics_slice() {
  // Given: a single logic snapshot
  let logic = LogicSnapshot::new(42, "only", Attributes::new());
  let interp = UninitializedInterpreter::new(vec![logic]);

  // When: accessed through the InterpreterSnapshot trait
  fn count<I: InterpreterSnapshot>(i: &I) -> usize {
    i.logics().len()
  }

  // Then: the trait dispatch returns the correct length
  assert_eq!(count(&interp), 1);
}

#[test]
fn interpreter_snapshot_is_object_safe() {
  // Given: a boxed trait object referencing the concrete implementation
  let interp: Box<dyn InterpreterSnapshot> =
    Box::new(UninitializedInterpreter::new(vec![LogicSnapshot::new(7, "boxed", Attributes::new())]));

  // Then: trait methods are dispatchable through the trait object
  assert_eq!(interp.logics().len(), 1);
  assert_eq!(interp.logics()[0].index(), 7);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn clone_preserves_logic_snapshots() {
  // Given: an interpreter with one logic
  let original = UninitializedInterpreter::new(vec![LogicSnapshot::new(99, "clone-me", Attributes::new())]);

  // When: cloned
  let cloned = original.clone();

  // Then: both retain the same logic content
  assert_eq!(cloned.logics().len(), 1);
  assert_eq!(cloned.logics()[0].index(), 99);
  assert_eq!(original.logics().len(), 1);
}

#[test]
fn debug_format_identifies_type() {
  // Given: an empty interpreter
  let interp = UninitializedInterpreter::new(vec![]);

  // When: formatted with Debug
  let debug = alloc::format!("{interp:?}");

  // Then: the type name appears
  assert!(debug.contains("UninitializedInterpreter"));
}
