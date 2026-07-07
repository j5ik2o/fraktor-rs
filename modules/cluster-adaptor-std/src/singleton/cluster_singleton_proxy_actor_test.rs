use alloc::{string::String, vec};

use fraktor_cluster_core_kernel_rs::{
  membership::{DataCenter, MembershipVersion, NodeRecord, NodeStatus},
  singleton::{ClusterSingletonProxyConfig, ClusterSingletonProxyEffect},
};

use super::ClusterSingletonProxyActor;
use crate::membership::ClusterMembershipEventHook;

fn make_record(authority: &str, join_v: u64) -> NodeRecord {
  NodeRecord::new(
    String::from("node"),
    String::from(authority),
    NodeStatus::Up,
    MembershipVersion::new(join_v),
    String::from("1.0.0"),
    vec![],
  )
}

#[test]
fn proxy_actor_identifies_oldest_member() {
  let mut actor = ClusterSingletonProxyActor::<u64>::new(ClusterSingletonProxyConfig::new(), DataCenter::default());
  actor.on_membership_event(ClusterMembershipEventHook);

  let outcome = actor.identify(&[make_record("n1:4000", 1), make_record("n2:4000", 2)]);
  assert_eq!(actor.proxy().identified_location(), Some("n1:4000"));
  assert!(
    outcome.effects.is_empty()
      || outcome.effects.iter().any(|effect| matches!(effect, ClusterSingletonProxyEffect::Identify))
  );
}
