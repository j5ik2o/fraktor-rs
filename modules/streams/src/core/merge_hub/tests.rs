use crate::core::MergeHub;

#[test]
fn merge_hub_preserves_offer_order() {
  let mut hub = MergeHub::new();
  hub.offer(1_u32);
  hub.offer(2_u32);
  hub.offer(3_u32);

  assert_eq!(hub.poll(), Some(1_u32));
  assert_eq!(hub.poll(), Some(2_u32));
  assert_eq!(hub.poll(), Some(3_u32));
  assert_eq!(hub.poll(), None);
}
