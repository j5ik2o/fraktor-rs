use super::StashBuffer;

#[test]
fn stash_buffer_capacity_matches_constructor() {
  let stash = StashBuffer::<u32>::new(8);
  assert_eq!(stash.capacity(), 8);
}
