use fraktor_utils_rs::core::sync::ArcShared;

use super::DispatcherSender;
use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    messaging::AnyMessage,
  },
  dispatch::{dispatcher::DispatcherShared, mailbox::Mailbox},
};

#[test]
fn dispatcher_sender_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);
  let _ = sender;
}

#[test]
fn dispatcher_sender_send_enqueued() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let mut sender = DispatcherSender::new(dispatcher);

  let result = <DispatcherSender as ActorRefSender>::send(&mut sender, AnyMessage::new(42_u32));
  assert!(result.is_ok());
}

#[test]
fn dispatcher_sender_send_multiple_messages() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let mut sender = DispatcherSender::new(dispatcher);

  assert!(<DispatcherSender as ActorRefSender>::send(&mut sender, AnyMessage::new(1_u32)).is_ok());
  assert!(<DispatcherSender as ActorRefSender>::send(&mut sender, AnyMessage::new(2_u32)).is_ok());
  assert!(<DispatcherSender as ActorRefSender>::send(&mut sender, AnyMessage::new(3_u32)).is_ok());
}

#[test]
fn dispatcher_sender_sets_need_reschedule_when_running() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox.clone());
  let mut sender = DispatcherSender::new(dispatcher);

  mailbox.set_running();
  let outcome = <DispatcherSender as ActorRefSender>::send(&mut sender, AnyMessage::new(99_u32)).expect("send");

  assert!(matches!(outcome, SendOutcome::Delivered));
  assert!(mailbox.set_idle());
}
