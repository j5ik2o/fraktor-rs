use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  dispatcher::InlineExecutor,
  mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy},
  props::DispatcherConfig,
  spawn::SpawnError,
};

#[test]
fn dispatcher_config_records_deadlines() {
  let executor = ArcShared::new(InlineExecutor::new());
  let config = DispatcherConfig::from_executor(executor)
    .with_throughput_deadline(Some(Duration::from_millis(1)))
    .with_starvation_deadline(Some(Duration::from_millis(5)));

  assert_eq!(config.throughput_deadline(), Some(Duration::from_millis(1)));
  assert_eq!(config.starvation_deadline(), Some(Duration::from_millis(5)));
}

#[test]
fn dispatcher_config_builds_dispatcher_with_deadlines() {
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let config = DispatcherConfig::from_executor(executor)
    .with_throughput_deadline(Some(Duration::from_micros(1)))
    .with_starvation_deadline(Some(Duration::from_micros(2)));

  let dispatcher = config.build_dispatcher(mailbox).expect("dispatcher creation should succeed");
  // Dispatcher creation should succeed even when deadlines are set.
  let _ = dispatcher;
}

#[test]
fn dispatcher_config_rejects_block_strategy_with_inline_executor() {
  // InlineExecutor does not support blocking operations
  let executor = ArcShared::new(InlineExecutor::new());
  let config = DispatcherConfig::from_executor(executor);

  // Create a bounded mailbox with Block overflow strategy
  let capacity = NonZeroUsize::new(10).expect("capacity should be non-zero");
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::Block, None);
  let mailbox = ArcShared::new(Mailbox::new(policy));

  // build_dispatcher should return an error
  let result = config.build_dispatcher(mailbox);
  assert!(result.is_err());

  if let Err(SpawnError::InvalidMailboxConfig(msg)) = result {
    assert!(
      msg.contains("Block") && msg.contains("blocking"),
      "エラーメッセージにBlockとblockingが含まれること: {}",
      msg
    );
  } else {
    panic!("InvalidMailboxConfigエラーが返されること");
  }
}

#[test]
fn dispatcher_config_accepts_drop_strategy_with_inline_executor() {
  // InlineExecutor でも DropNewest/DropOldest は使用可能
  let executor = ArcShared::new(InlineExecutor::new());
  let config = DispatcherConfig::from_executor(executor);

  let capacity = NonZeroUsize::new(10).expect("capacity should be non-zero");
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let mailbox = ArcShared::new(Mailbox::new(policy));

  // build_dispatcher should succeed with DropNewest
  let result = config.build_dispatcher(mailbox);
  assert!(result.is_ok(), "DropNewest戦略では成功すること");
}
