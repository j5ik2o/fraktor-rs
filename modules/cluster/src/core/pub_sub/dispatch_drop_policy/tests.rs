use crate::core::pub_sub::dispatch_drop_policy::DispatchDropPolicy;

#[test]
fn drop_oldest_and_reject_new_are_distinct() {
  assert_ne!(DispatchDropPolicy::DropOldest, DispatchDropPolicy::RejectNew);
}
