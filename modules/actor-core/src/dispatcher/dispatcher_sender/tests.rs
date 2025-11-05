use cellactor_utils_core_rs::sync::ArcShared;

use super::DispatcherSender;
use crate::{
  NoStdToolbox, actor_prim::actor_ref::ActorRefSender, dispatcher::base::Dispatcher, mailbox::Mailbox,
  messaging::AnyMessage,
};

#[test]
fn dispatcher_sender_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = Dispatcher::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);
  let _ = sender;
}

#[test]
fn dispatcher_sender_send_enqueued() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = Dispatcher::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);

  let result = <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessage::new(42_u32));
  assert!(result.is_ok());
}

#[test]
fn dispatcher_sender_send_multiple_messages() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = Dispatcher::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);

  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessage::new(1_u32)).is_ok()
  );
  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessage::new(2_u32)).is_ok()
  );
  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessage::new(3_u32)).is_ok()
  );
}
