use crate::core::PartitionHub;

#[test]
fn partition_hub_routes_values_to_selected_partitions() {
  let mut hub = PartitionHub::new(2);
  hub.offer(0, 1_u32);
  hub.offer(1, 2_u32);
  hub.offer(0, 3_u32);

  assert_eq!(hub.poll(0), Some(1_u32));
  assert_eq!(hub.poll(0), Some(3_u32));
  assert_eq!(hub.poll(1), Some(2_u32));
}
