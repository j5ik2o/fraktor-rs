use fraktor_streams_rs::core::{BroadcastHub, MergeHub, PartitionHub, SharedKillSwitch, StreamError, UniqueKillSwitch};

#[test]
fn dynamic_hub_contract_returns_would_block_without_active_consumers() {
  let merge_hub = MergeHub::new();
  assert_eq!(merge_hub.offer(1_u32), Err(StreamError::WouldBlock));

  let broadcast_hub = BroadcastHub::new();
  assert_eq!(broadcast_hub.publish(1_u32), Err(StreamError::WouldBlock));

  let partition_hub = PartitionHub::new(1);
  assert_eq!(partition_hub.offer(0, 1_u32), Err(StreamError::WouldBlock));
  assert_eq!(partition_hub.route_with(1_u32, |_| 0), Err(StreamError::WouldBlock));
}

#[test]
fn kill_switch_keeps_first_control_signal_for_unique_and_shared() {
  let unique = UniqueKillSwitch::new();
  unique.shutdown();
  unique.abort(StreamError::Failed);
  assert!(unique.is_shutdown());
  assert!(!unique.is_aborted());
  assert_eq!(unique.abort_error(), None);

  let shared = SharedKillSwitch::new();
  let shared_clone = shared.clone();
  shared_clone.abort(StreamError::Failed);
  shared.shutdown();
  assert!(shared.is_aborted());
  assert!(!shared.is_shutdown());
  assert_eq!(shared.abort_error(), Some(StreamError::Failed));
}
