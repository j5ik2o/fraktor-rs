use std::collections::BTreeMap;

use fraktor_cluster_rs::core::{GrainKey, IdentityLookup, PartitionIdentityLookup, PlacementLocality};

#[test]
fn partition_identity_lookup_distributes_keys_across_members() {
  let authorities = vec!["node-a:4050".to_string(), "node-b:4051".to_string(), "node-c:4052".to_string()];
  let mut lookup = PartitionIdentityLookup::with_defaults();
  lookup.setup_member(&[]).expect("setup_member");
  lookup.update_topology(authorities.clone());
  lookup.set_local_authority("node-a:4050");

  let total_keys = 600_usize;
  let mut counts = BTreeMap::<String, usize>::new();
  for index in 0..total_keys {
    let key = GrainKey::new(format!("user/{index}"));
    let resolution = lookup.resolve(&key, index as u64).expect("resolve");
    *counts.entry(resolution.decision.authority.clone()).or_insert(0) += 1;

    if resolution.decision.authority == "node-a:4050" {
      assert_eq!(resolution.locality, PlacementLocality::Local);
    } else {
      assert_eq!(resolution.locality, PlacementLocality::Remote);
    }
  }

  assert_eq!(counts.len(), authorities.len(), "all authorities should receive assignments");
  for authority in &authorities {
    let assigned = *counts.get(authority).unwrap_or(&0);
    assert!(
      (120..=280).contains(&assigned),
      "authority={authority} assigned={assigned} is outside expected load-balanced range"
    );
  }
}
