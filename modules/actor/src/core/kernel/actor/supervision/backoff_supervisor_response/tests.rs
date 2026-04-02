use crate::core::kernel::actor::{Pid, supervision::BackoffSupervisorResponse};

#[test]
fn current_child_with_some_pid() {
  // Given: a valid pid
  let pid = Pid::new(42, 0);

  // When: wrapping in CurrentChild
  let response = BackoffSupervisorResponse::CurrentChild(Some(pid));

  // Then: the pid is accessible
  match response {
    | BackoffSupervisorResponse::CurrentChild(Some(p)) => assert_eq!(p, pid),
    | _ => panic!("expected CurrentChild(Some(_))"),
  }
}

#[test]
fn current_child_with_none() {
  // Given/When: constructing CurrentChild with no child
  let response = BackoffSupervisorResponse::CurrentChild(None);

  // Then: it matches None variant
  assert!(matches!(response, BackoffSupervisorResponse::CurrentChild(None)));
}

#[test]
fn restart_count_zero() {
  // Given/When: constructing RestartCount with zero
  let response = BackoffSupervisorResponse::RestartCount(0);

  // Then: count is zero
  match response {
    | BackoffSupervisorResponse::RestartCount(count) => assert_eq!(count, 0),
    | _ => panic!("expected RestartCount"),
  }
}

#[test]
fn restart_count_nonzero() {
  // Given/When: constructing RestartCount with a value
  let response = BackoffSupervisorResponse::RestartCount(5);

  // Then: count matches
  match response {
    | BackoffSupervisorResponse::RestartCount(count) => assert_eq!(count, 5),
    | _ => panic!("expected RestartCount"),
  }
}

#[test]
fn response_variants_are_distinct() {
  // Given: both response variants
  let child = BackoffSupervisorResponse::CurrentChild(None);
  let count = BackoffSupervisorResponse::RestartCount(0);

  // Then: each does not match the other
  assert!(!matches!(child, BackoffSupervisorResponse::RestartCount(_)));
  assert!(!matches!(count, BackoffSupervisorResponse::CurrentChild(_)));
}

#[test]
fn response_clone_preserves_value() {
  // Given: a response with a pid
  let pid = Pid::new(10, 0);
  let original = BackoffSupervisorResponse::CurrentChild(Some(pid));

  // When: cloned
  let cloned = original.clone();

  // Then: clone contains the same pid
  match cloned {
    | BackoffSupervisorResponse::CurrentChild(Some(p)) => assert_eq!(p, pid),
    | _ => panic!("expected CurrentChild(Some(_))"),
  }
}

#[test]
fn response_debug_format_is_non_empty() {
  // Given: a response variant
  let response = BackoffSupervisorResponse::RestartCount(3);

  // When: formatted with Debug
  let debug_str = alloc::format!("{:?}", response);

  // Then: the output is non-empty
  assert!(!debug_str.is_empty());
}
