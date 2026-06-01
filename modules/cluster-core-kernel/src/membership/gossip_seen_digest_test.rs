use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{GossipSeenDigest, MembershipVersion};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn mark_seen_reports_change_when_inserting_zero_version_peer() {
  let peer = unique_address("node-a", 10);
  let mut digest = GossipSeenDigest::new();

  assert!(digest.mark_seen(peer.clone(), MembershipVersion::zero()));
  assert_eq!(digest.observed_version(&peer), Some(MembershipVersion::zero()));
  assert!(!digest.mark_seen(peer, MembershipVersion::zero()));
}
