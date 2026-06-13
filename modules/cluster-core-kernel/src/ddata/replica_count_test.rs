use crate::ddata::{FlushChanges, GetReplicaCount, ReplicaCount};

#[test]
fn replica_count_returns_inner_value() {
  assert_eq!(ReplicaCount::new(3).get(), 3);
}

#[test]
fn auxiliary_protocol_values_are_constructible() {
  let get_replica_count = GetReplicaCount;
  let flush_changes = FlushChanges;

  assert_eq!(get_replica_count, GetReplicaCount);
  assert_eq!(flush_changes, FlushChanges);
}
