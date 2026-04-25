use alloc::string::String;
use core::error::Error;

use super::ExecuteError;

#[test]
fn display_matches_public_contract() {
  assert_eq!(alloc::format!("{}", ExecuteError::Rejected), "executor rejected the submitted task");
  assert_eq!(alloc::format!("{}", ExecuteError::Shutdown), "executor is shut down");
  assert_eq!(
    alloc::format!("{}", ExecuteError::Backend("queue saturated".into())),
    "executor backend error: queue saturated"
  );
}

#[test]
fn implements_core_error_trait() {
  fn format_via_dyn_error(error: &dyn Error) -> String {
    alloc::format!("{error}")
  }

  assert_eq!(format_via_dyn_error(&ExecuteError::Shutdown), "executor is shut down");
}
