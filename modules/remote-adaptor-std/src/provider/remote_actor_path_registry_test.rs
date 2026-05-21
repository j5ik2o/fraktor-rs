use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathParser},
  actor_ref::{ActorRefSenderShared, NullSender},
};

use super::RemoteActorPathRegistry;

fn test_actor_ref_sender() -> ActorRefSenderShared {
  ActorRefSenderShared::new(Box::new(NullSender))
}

fn remote_actor_path(name: &str) -> ActorPath {
  ActorPathParser::parse(&format!("fraktor.tcp://remote-sys@10.0.0.1:2552/user/{name}")).expect("parse")
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "remote path registry order contained pid")]
fn debug_asserts_order_path_mismatch() {
  let mut registry = RemoteActorPathRegistry::default();
  let mut senders = Vec::new();
  let missing_pid = Pid::new(5000, 0);

  for index in 0..1024 {
    let path = remote_actor_path(&format!("debug-{index}"));
    let sender = test_actor_ref_sender();
    assert!(registry.record(Pid::new(5000 + index, 0), path, &sender));
    senders.push(sender);
  }

  registry.paths.remove(&missing_pid);
  let replacement_sender = test_actor_ref_sender();
  assert!(registry.record(Pid::new(7000, 0), remote_actor_path("debug-replacement"), &replacement_sender));
  senders.push(replacement_sender);

  let overflow_sender = test_actor_ref_sender();
  let overflow_path = remote_actor_path("debug-overflow");
  registry.record(Pid::new(7001, 0), overflow_path, &overflow_sender);
}
