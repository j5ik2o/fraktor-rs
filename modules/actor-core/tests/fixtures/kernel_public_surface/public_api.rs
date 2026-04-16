use fraktor_actor_core_rs::core::kernel::{
  actor::{
    ActorCell, ActorContext, Address, ChildRef, ClassicTimerScheduler, Pid,
    actor_path::{ActorPath, ChildActorPath, RootActorPath},
    actor_ref::ActorRef,
    actor_selection::ActorSelection,
    messaging::{ActorIdentity, Identify, Kill, PoisonPill, ReceiveTimeout},
    props::Props,
  },
  dispatch::mailbox::Mailbox,
  routing::{Broadcast, RoundRobinRoutingLogic, Routee, Router},
  system::{ActorSystem, CoordinatedShutdown, CoordinatedShutdownPhase, shared_factory::MailboxSharedSet},
};

fn main() {
  let _ = core::any::type_name::<ActorCell>();
  let _ = core::any::type_name::<ActorContext<'static>>();
  let _ = core::any::type_name::<Address>();
  let _ = core::any::type_name::<ChildRef>();
  let _ = core::any::type_name::<ClassicTimerScheduler>();
  let _ = core::any::type_name::<Pid>();
  let _ = core::any::type_name::<ActorPath>();
  let _ = core::any::type_name::<RootActorPath>();
  let _ = core::any::type_name::<ChildActorPath>();
  let _ = core::any::type_name::<ActorRef>();
  let _ = core::any::type_name::<ActorSelection>();
  let _ = core::any::type_name::<ActorIdentity>();
  let _ = core::any::type_name::<Identify>();
  let _ = core::any::type_name::<Kill>();
  let _ = core::any::type_name::<PoisonPill>();
  let _ = core::any::type_name::<ReceiveTimeout>();
  let _ = core::any::type_name::<Props>();
  let _ = core::any::type_name::<Broadcast>();
  let _ = core::any::type_name::<Routee>();
  let _ = core::any::type_name::<Router<RoundRobinRoutingLogic>>();
  let _ = core::any::type_name::<ActorSystem>();
  let _ = core::any::type_name::<CoordinatedShutdown>();
  let _ = core::any::type_name::<CoordinatedShutdownPhase>();
  let _ = core::any::type_name::<Mailbox>();
  let _ = core::any::type_name::<MailboxSharedSet>();
}
