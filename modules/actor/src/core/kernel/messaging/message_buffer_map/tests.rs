use crate::core::kernel::{
  actor::actor_ref::ActorRef,
  messaging::{AnyMessage, MessageBufferMap},
};

#[test]
fn empty_map_is_empty() {
  let map: MessageBufferMap<u32> = MessageBufferMap::empty();
  assert!(map.is_empty());
  assert_eq!(map.size(), 0);
  assert_eq!(map.total_size(), 0);
}

#[test]
fn add_creates_empty_buffer() {
  let mut map: MessageBufferMap<u32> = MessageBufferMap::empty();
  map.add(1);
  assert!(map.contains(&1));
  assert_eq!(map.size(), 1);
  assert_eq!(map.total_size(), 0);
}

#[test]
fn append_adds_to_correct_buffer() {
  let mut map: MessageBufferMap<&str> = MessageBufferMap::empty();
  map.append("a", AnyMessage::new(10_u32), ActorRef::null());
  map.append("a", AnyMessage::new(20_u32), ActorRef::null());
  map.append("b", AnyMessage::new(30_u32), ActorRef::null());

  assert_eq!(map.size(), 2);
  assert_eq!(map.total_size(), 3);

  let buf_a = map.get(&"a").expect("buffer for 'a'");
  assert_eq!(buf_a.size(), 2);

  let buf_b = map.get(&"b").expect("buffer for 'b'");
  assert_eq!(buf_b.size(), 1);
}

#[test]
fn remove_deletes_buffer() {
  let mut map: MessageBufferMap<u32> = MessageBufferMap::empty();
  map.append(1, AnyMessage::new(42_u32), ActorRef::null());
  assert!(map.contains(&1));

  map.remove(&1);
  assert!(!map.contains(&1));
  assert!(map.is_empty());
}

#[test]
fn contains_returns_false_for_absent_id() {
  let map: MessageBufferMap<u32> = MessageBufferMap::empty();
  assert!(!map.contains(&99));
}

#[test]
fn get_returns_none_for_absent_id() {
  let map: MessageBufferMap<u32> = MessageBufferMap::empty();
  assert!(map.get(&99).is_none());
}

#[test]
fn for_each_visits_all_entries() {
  let mut map: MessageBufferMap<u32> = MessageBufferMap::empty();
  map.append(1, AnyMessage::new(10_u32), ActorRef::null());
  map.append(2, AnyMessage::new(20_u32), ActorRef::null());

  let mut visited = alloc::vec::Vec::new();
  map.for_each(|id, buf| {
    visited.push((*id, buf.size()));
  });
  visited.sort_by_key(|(id, _)| *id);
  assert_eq!(visited, alloc::vec![(1, 1), (2, 1)]);
}

#[test]
fn total_size_sums_across_all_buffers() {
  let mut map: MessageBufferMap<u32> = MessageBufferMap::empty();
  map.append(1, AnyMessage::new(1_u32), ActorRef::null());
  map.append(1, AnyMessage::new(2_u32), ActorRef::null());
  map.append(2, AnyMessage::new(3_u32), ActorRef::null());

  assert_eq!(map.total_size(), 3);

  map.remove(&1);
  assert_eq!(map.total_size(), 1);
}

#[test]
fn default_creates_empty_map() {
  let map: MessageBufferMap<u32> = MessageBufferMap::default();
  assert!(map.is_empty());
}
