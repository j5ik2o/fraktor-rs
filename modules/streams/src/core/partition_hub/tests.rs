use crate::core::PartitionHub;

#[test]
fn partition_hub_routes_values_to_selected_partitions() {
  let hub = PartitionHub::new(2);
  hub.offer(0, 1_u32);
  hub.offer(1, 2_u32);
  hub.offer(0, 3_u32);

  assert_eq!(hub.poll(0), Some(1_u32));
  assert_eq!(hub.poll(0), Some(3_u32));
  assert_eq!(hub.poll(1), Some(2_u32));
}

#[test]
fn partition_hub_source_for_drains_selected_partition() {
  let hub = PartitionHub::new(2);
  hub.offer(0, 7_u32);
  hub.offer(1, 8_u32);
  hub.offer(0, 9_u32);

  let values = hub.source_for(0).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 9_u32]);
}
