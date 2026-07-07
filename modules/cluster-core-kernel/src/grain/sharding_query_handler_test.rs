use alloc::{collections::BTreeSet, string::ToString};

use crate::{
  activation::VirtualActorRegistry,
  grain::{GrainKey, ShardingQuery, ShardingQueryHandler, ShardingQueryResponse},
};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

#[test]
fn get_shard_region_state_groups_entities_by_shard_prefix() {
  let mut registry = VirtualActorRegistry::new(8, 60);
  registry.ensure_activation(&key("0:user-1"), &["node:4000".to_string()], 1, false, None).expect("activation");
  registry.ensure_activation(&key("1:user-2"), &["node:4000".to_string()], 1, false, None).expect("activation");

  let regions = BTreeSet::from(["node:4000".to_string()]);
  let handler = ShardingQueryHandler::new(&registry, "node:4000".to_string(), &regions);
  let response = handler.handle(ShardingQuery::GetShardRegionState);

  match response {
    | ShardingQueryResponse::CurrentShardRegionState { shards, failed } => {
      assert!(failed.is_empty());
      assert_eq!(shards.len(), 2);
    },
    | _ => panic!("expected CurrentShardRegionState"),
  }
}

#[test]
fn get_current_regions_returns_registered_regions() {
  let registry = VirtualActorRegistry::new(4, 60);
  let regions = BTreeSet::from(["node:4000".to_string(), "node:4001".to_string()]);
  let handler = ShardingQueryHandler::new(&registry, "node:4000".to_string(), &regions);
  let response = handler.handle(ShardingQuery::GetCurrentRegions);

  match response {
    | ShardingQueryResponse::CurrentRegions { regions } => {
      assert_eq!(regions.len(), 2);
    },
    | _ => panic!("expected CurrentRegions"),
  }
}
