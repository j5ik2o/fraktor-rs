use crate::core::kernel::actor::{
  Pid,
  actor_ref::NullSender,
  messaging::{ActorIdentity, AnyMessage, actor_identity::ActorRef},
};

#[test]
fn actor_identity_keeps_correlation_id_and_actor_ref() {
  let actor_ref = ActorRef::new(Pid::new(10, 0), NullSender);
  let identity = ActorIdentity::found(AnyMessage::new("corr"), actor_ref.clone());

  let correlation_id = identity.correlation_id().payload().downcast_ref::<&str>().expect("&str");
  assert_eq!(*correlation_id, "corr");
  assert_eq!(identity.actor_ref().expect("actor ref").pid(), actor_ref.pid());
}
