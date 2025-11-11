use core::time::Duration;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  dispatcher::InlineExecutor,
  mailbox::{Mailbox, MailboxPolicy},
  props::DispatcherConfig,
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

  let dispatcher = config.build_dispatcher(mailbox);
  // Dispatcher creation should succeed even when deadlines are set.
  let _ = dispatcher;
}
