use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::StashBufferGeneric;

#[test]
fn stash_buffer_capacity_matches_constructor() {
  let stash = StashBufferGeneric::<u32, NoStdToolbox>::new(8);
  assert_eq!(stash.capacity(), 8);
}
