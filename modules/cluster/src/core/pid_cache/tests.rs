use crate::core::{grain_key::GrainKey, pid_cache::PidCache, pid_cache_event::PidCacheEvent};

fn key(v: &str) -> GrainKey {
  GrainKey::new(v.to_string())
}

#[test]
fn returns_entry_before_expiry_and_drops_after() {
  let mut cache = PidCache::new(4);
  cache.put(key("k1"), "pid-1".to_string(), "a1".to_string(), 10, 5);

  assert_eq!(cache.get(&key("k1"), 12), Some("pid-1".to_string()));
  assert!(cache.get(&key("k1"), 16).is_none());

  let events = cache.drain_events();
  assert_eq!(events.len(), 1);
  assert!(matches!(events[0], PidCacheEvent::Dropped { .. }));
}

#[test]
fn quarantine_invalidation_drops_entries() {
  let mut cache = PidCache::new(4);
  cache.put(key("k1"), "pid-1".to_string(), "a1".to_string(), 0, 100);
  cache.put(key("k2"), "pid-2".to_string(), "a2".to_string(), 0, 100);

  cache.invalidate_authority("a1");

  assert!(cache.get(&key("k1"), 1).is_none());
  assert_eq!(cache.get(&key("k2"), 1), Some("pid-2".to_string()));

  let events = cache.drain_events();
  assert!(events.iter().any(|e| matches!(e, PidCacheEvent::Dropped { reason, .. } if reason == "quarantine")));
}

#[test]
fn evicts_when_capacity_is_reached() {
  let mut cache = PidCache::new(1);
  cache.put(key("k1"), "pid-1".to_string(), "a1".to_string(), 0, 100);
  cache.put(key("k2"), "pid-2".to_string(), "a1".to_string(), 0, 100);

  assert!(cache.get(&key("k1"), 1).is_none());
  assert_eq!(cache.get(&key("k2"), 1), Some("pid-2".to_string()));
}
