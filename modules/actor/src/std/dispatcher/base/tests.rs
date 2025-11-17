extern crate std;

use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::{
  core::dispatcher::{DispatchError, DispatchExecutor as CoreDispatchExecutor},
  std::dispatcher::{DispatchExecutor, DispatchShared},
};

struct TestExecutor {
  executed: ArcShared<std::sync::Mutex<bool>>,
}

impl TestExecutor {
  fn new() -> (Self, ArcShared<std::sync::Mutex<bool>>) {
    let executed = ArcShared::new(std::sync::Mutex::new(false));
    (Self { executed: executed.clone() }, executed)
  }

  fn lock_state(state: &ArcShared<std::sync::Mutex<bool>>) -> std::sync::MutexGuard<'_, bool> {
    match state.lock() {
      | Ok(guard) => guard,
      | Err(poisoned) => poisoned.into_inner(),
    }
  }
}

impl CoreDispatchExecutor<StdToolbox> for TestExecutor {
  fn execute(&self, _dispatcher: DispatchShared) -> Result<(), DispatchError> {
    *Self::lock_state(&self.executed) = true;
    Ok(())
  }
}

#[test]
fn dispatch_executor_trait_implemented() {
  let (executor, executed) = TestExecutor::new();
  let executor_dyn: &dyn DispatchExecutor = &executor;

  // This test just verifies trait implementation compiles correctly
  let _ = executor_dyn;
  assert!(!*TestExecutor::lock_state(&executed));
}

#[test]
fn core_dispatch_executor_trait_implemented() {
  let (executor, executed) = TestExecutor::new();
  let executor_dyn: &dyn CoreDispatchExecutor<StdToolbox> = &executor;

  // This test just verifies trait implementation compiles correctly
  let _ = executor_dyn;
  assert!(!*TestExecutor::lock_state(&executed));
}
