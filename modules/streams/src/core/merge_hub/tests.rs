use crate::core::MergeHub;

#[test]
fn merge_hub_preserves_offer_order() {
  let hub = MergeHub::new();
  hub.offer(1_u32);
  hub.offer(2_u32);
  hub.offer(3_u32);

  assert_eq!(hub.poll(), Some(1_u32));
  assert_eq!(hub.poll(), Some(2_u32));
  assert_eq!(hub.poll(), Some(3_u32));
  assert_eq!(hub.poll(), None);
}

#[test]
fn merge_hub_source_drains_as_stream_source() {
  let hub = MergeHub::new();
  hub.offer(10_u32);
  hub.offer(20_u32);

  let values = hub.source().collect_values().expect("collect_values");
  assert_eq!(values, vec![10_u32, 20_u32]);
}
