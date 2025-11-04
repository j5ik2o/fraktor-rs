use cellactor_utils_core_rs::sync::ArcShared;

use super::DispatcherSender;
use crate::{
  NoStdToolbox, actor_prim::actor_ref::ActorRefSender, dispatcher::base::DispatcherGeneric, mailbox::MailboxGeneric,
  messaging::AnyMessageGeneric,
};

#[test]
fn dispatcher_sender_new() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherGeneric::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);
  let _ = sender;
}

#[test]
fn dispatcher_sender_send_enqueued() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherGeneric::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);

  let result =
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessageGeneric::new(42_u32));
  assert!(result.is_ok());
}

#[test]
fn dispatcher_sender_send_multiple_messages() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let dispatcher = DispatcherGeneric::with_inline_executor(mailbox);
  let sender = DispatcherSender::new(dispatcher);

  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessageGeneric::new(1_u32))
      .is_ok()
  );
  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessageGeneric::new(2_u32))
      .is_ok()
  );
  assert!(
    <DispatcherSender<NoStdToolbox> as ActorRefSender<NoStdToolbox>>::send(&sender, AnyMessageGeneric::new(3_u32))
      .is_ok()
  );
}
