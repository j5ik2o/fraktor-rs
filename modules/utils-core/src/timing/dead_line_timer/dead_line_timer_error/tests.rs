use super::DeadLineTimerError;

#[test]
fn display_variants() {
  assert_eq!(format!("{}", DeadLineTimerError::Full), "deadline timer is full");
  assert_eq!(format!("{}", DeadLineTimerError::NotFound), "key not found");
  assert_eq!(format!("{}", DeadLineTimerError::BackendFailure), "backend failure");
}
