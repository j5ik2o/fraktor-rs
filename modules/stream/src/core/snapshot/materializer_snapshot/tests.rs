use crate::core::{materialization::MaterializerLifecycleState, snapshot::MaterializerSnapshot};

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_snapshot_preserves_lifecycle_state() {
  // Given: a snapshot created with Idle state
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Idle, 0);

  // Then: lifecycle_state returns Idle
  assert_eq!(snapshot.lifecycle_state(), MaterializerLifecycleState::Idle);
}

#[test]
fn new_snapshot_preserves_total_materialized() {
  // Given: a snapshot created with total_materialized = 42
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Running, 42);

  // Then: total_materialized returns 42
  assert_eq!(snapshot.total_materialized(), 42);
}

// ---------------------------------------------------------------------------
// State combinations
// ---------------------------------------------------------------------------

#[test]
fn snapshot_with_idle_and_zero_count() {
  // Given: an idle materializer that has not materialized any graphs
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Idle, 0);

  // Then: reflects the initial state
  assert_eq!(snapshot.lifecycle_state(), MaterializerLifecycleState::Idle);
  assert_eq!(snapshot.total_materialized(), 0);
}

#[test]
fn snapshot_with_running_and_nonzero_count() {
  // Given: a running materializer that has materialized 5 graphs
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Running, 5);

  // Then: reflects the active state
  assert_eq!(snapshot.lifecycle_state(), MaterializerLifecycleState::Running);
  assert_eq!(snapshot.total_materialized(), 5);
}

#[test]
fn snapshot_with_stopped_state() {
  // Given: a stopped materializer
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Stopped, 10);

  // Then: reflects the stopped state
  assert_eq!(snapshot.lifecycle_state(), MaterializerLifecycleState::Stopped);
  assert_eq!(snapshot.total_materialized(), 10);
}

// ---------------------------------------------------------------------------
// Derive trait verification
// ---------------------------------------------------------------------------

#[test]
fn debug_format_contains_fields() {
  // Given: a snapshot
  let snapshot = MaterializerSnapshot::new(MaterializerLifecycleState::Running, 3);

  // When: formatted with Debug
  let debug = format!("{snapshot:?}");

  // Then: output contains both field values
  assert!(debug.contains("Running"));
  assert!(debug.contains("3"));
}

#[test]
fn clone_produces_equal_snapshot() {
  // Given: a snapshot
  let original = MaterializerSnapshot::new(MaterializerLifecycleState::Running, 7);

  // When: cloned
  let cloned = original.clone();

  // Then: the clone has the same values
  assert_eq!(cloned.lifecycle_state(), MaterializerLifecycleState::Running);
  assert_eq!(cloned.total_materialized(), 7);
}
