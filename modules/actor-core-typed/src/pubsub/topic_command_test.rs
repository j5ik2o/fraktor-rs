use crate::pubsub::{TopicCommand, TopicStats};

#[test]
fn topic_command_is_accessible() {
  let _ = core::mem::size_of::<TopicCommand<u32>>();
  let _ = core::mem::size_of::<TopicStats>();
}
