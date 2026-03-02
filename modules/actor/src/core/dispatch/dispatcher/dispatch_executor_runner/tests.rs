use alloc::{boxed::Box, sync::Arc, vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::DispatchExecutorRunner;
use crate::core::dispatch::dispatcher::{
  DispatchError, DispatchExecutor, DispatchShared, dispatcher_core::DispatcherCore,
};

fn make_dispatch_task() -> DispatchShared {
  use crate::core::dispatch::{
    dispatcher::InlineExecutor,
    mailbox::{Mailbox, MailboxPolicy},
  };

  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let inline_executor: Box<dyn DispatchExecutor> = Box::new(InlineExecutor::new());
  let inner_runner = ArcShared::new(DispatchExecutorRunner::new(inline_executor));
  let adapter = crate::core::dispatch::dispatcher::InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox, inner_runner, adapter, None, None, None));

  DispatchShared::new(core)
}

struct CountingExecutor {
  count: Arc<AtomicUsize>,
}

impl CountingExecutor {
  fn new(count: Arc<AtomicUsize>) -> Self {
    Self { count }
  }
}

impl DispatchExecutor for CountingExecutor {
  fn execute(&mut self, _dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.count.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  fn supports_blocking(&self) -> bool {
    false
  }
}

#[test]
fn dispatch_executor_runner_executes_single_task() {
  let count = Arc::new(AtomicUsize::new(0));
  let executor: Box<dyn DispatchExecutor> = Box::new(CountingExecutor::new(count.clone()));
  let runner = DispatchExecutorRunner::new(executor);

  let task = make_dispatch_task();

  runner.submit(task).expect("submit should succeed");

  assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn dispatch_executor_runner_supports_blocking_delegates() {
  let count = Arc::new(AtomicUsize::new(0));
  let executor: Box<dyn DispatchExecutor> = Box::new(CountingExecutor::new(count));
  let runner = DispatchExecutorRunner::new(executor);

  assert!(!runner.supports_blocking());
}

struct SequenceExecutor {
  outcomes: Arc<NoStdMutex<Vec<Result<(), DispatchError>>>>,
  calls:    Arc<AtomicUsize>,
}

impl SequenceExecutor {
  fn new(outcomes: Arc<NoStdMutex<Vec<Result<(), DispatchError>>>>, calls: Arc<AtomicUsize>) -> Self {
    Self { outcomes, calls }
  }
}

impl DispatchExecutor for SequenceExecutor {
  fn execute(&mut self, _dispatcher: DispatchShared) -> Result<(), DispatchError> {
    self.calls.fetch_add(1, Ordering::SeqCst);
    self.outcomes.lock().pop().unwrap_or(Ok(()))
  }

  fn supports_blocking(&self) -> bool {
    false
  }
}

#[test]
fn failed_task_is_requeued_and_retried() {
  // 先頭のタスクで拒否されても再度キューへ戻されることを確認する
  let outcomes = Arc::new(NoStdMutex::new(vec![Ok(()), Err(DispatchError::RejectedExecution)]));
  let calls = Arc::new(AtomicUsize::new(0));
  let executor: Box<dyn DispatchExecutor> = Box::new(SequenceExecutor::new(outcomes, calls.clone()));
  let runner = DispatchExecutorRunner::new(executor);

  let task1 = make_dispatch_task();
  let err = runner.submit(task1).expect_err("first submit should propagate rejection");
  assert!(matches!(err, DispatchError::RejectedExecution));

  let task2 = make_dispatch_task();
  runner.submit(task2).expect("second submit should succeed after retry");

  // 失敗→リトライ→新規タスクの順で 3 回呼ばれるはず
  assert_eq!(calls.load(Ordering::SeqCst), 3);
}
