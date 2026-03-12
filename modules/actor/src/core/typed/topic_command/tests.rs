use crate::core::typed::{TopicCommand, TopicStats};

#[test]
fn topic_command_is_accessible() {
  let _: fn() -> usize = || core::mem::size_of::<TopicCommand<u32>>();
  let _: fn() -> usize = || core::mem::size_of::<TopicStats>();
}
