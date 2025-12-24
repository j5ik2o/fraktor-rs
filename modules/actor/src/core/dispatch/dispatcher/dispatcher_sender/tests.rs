use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::DispatcherSenderGeneric;
use crate::core::{
  actor_prim::actor_ref::{ActorRefSender, SendOutcome},
  dispatch::{dispatcher::base::DispatcherShared, mailbox::Mailbox},
  messaging::AnyMessage,
};

#[test]
fn dispatcher_sender_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let sender = DispatcherSenderGeneric::new(dispatcher);
  let _ = sender;
}

#[test]
fn dispatcher_sender_send_enqueued() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let mut sender = DispatcherSenderGeneric::new(dispatcher);

  let result =
    <DispatcherSenderGeneric<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&mut sender, AnyMessage::new(42_u32));
  assert!(result.is_ok());
}

#[test]
fn dispatcher_sender_send_multiple_messages() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox);
  let mut sender = DispatcherSenderGeneric::new(dispatcher);

  assert!(
    <DispatcherSenderGeneric<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&mut sender, AnyMessage::new(1_u32))
      .is_ok()
  );
  assert!(
    <DispatcherSenderGeneric<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&mut sender, AnyMessage::new(2_u32))
      .is_ok()
  );
  assert!(
    <DispatcherSenderGeneric<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&mut sender, AnyMessage::new(3_u32))
      .is_ok()
  );
}

#[test]
fn dispatcher_sender_sets_need_reschedule_when_running() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherShared::with_inline_executor(mailbox.clone());
  let mut sender = DispatcherSenderGeneric::new(dispatcher);

  mailbox.set_running();
  let outcome =
    <DispatcherSenderGeneric<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&mut sender, AnyMessage::new(99_u32))
      .expect("send");

  assert!(matches!(outcome, SendOutcome::Delivered));
  assert!(mailbox.set_idle());
}
