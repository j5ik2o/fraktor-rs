use alloc::{string::String, vec};

use super::{ClusterSingletonProxy, ClusterSingletonProxyEffect};
use crate::{
  membership::{DataCenter, MembershipVersion, NodeRecord, NodeStatus},
  singleton::ClusterSingletonProxyConfig,
};

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
fn identify_forwards_buffered_messages_to_oldest_member() {
  let mut proxy = ClusterSingletonProxy::<u64>::new(ClusterSingletonProxyConfig::new().with_buffer_size(10));
  let _ = proxy.handle_message(42);
  assert_eq!(proxy.buffered_count(), 1);

  let members = vec![make_record("n1:4000", 1), make_record("n2:4000", 2)];
  let outcome = proxy.identify(&members, &DataCenter::default());

  assert_eq!(proxy.identified_location(), Some("n1:4000"));
  assert_eq!(proxy.buffered_count(), 0);
  assert_eq!(outcome.effects, vec![ClusterSingletonProxyEffect::Forward {
    location: String::from("n1:4000"),
    message:  42,
  }]);
}

#[test]
fn zero_buffer_size_drops_messages_when_unidentified() {
  let mut proxy = ClusterSingletonProxy::<&str>::new(ClusterSingletonProxyConfig::new().with_buffer_size(0));
  let outcome = proxy.handle_message("hello");
  assert_eq!(outcome.effects, vec![ClusterSingletonProxyEffect::Drop { message: "hello" }]);
}

#[test]
fn identified_proxy_forwards_immediately() {
  let mut proxy = ClusterSingletonProxy::<u64>::new(ClusterSingletonProxyConfig::new());
  let members = vec![make_record("n1:4000", 1)];
  let _ = proxy.identify(&members, &DataCenter::default());

  let outcome = proxy.handle_message(7);
  assert_eq!(outcome.effects, vec![ClusterSingletonProxyEffect::Forward {
    location: String::from("n1:4000"),
    message:  7,
  }]);
}
