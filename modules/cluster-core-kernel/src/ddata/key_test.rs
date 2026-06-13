use alloc::{collections::BTreeMap, string::String};
use core::hash::{Hash, Hasher};

use crate::ddata::{FlagKey, GCounterKey, Key, PNCounterKey, PNCounterMapKey};

#[derive(Default)]
struct StableHasher(u64);

impl Hasher for StableHasher {
  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, bytes: &[u8]) {
    for byte in bytes {
      self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(*byte));
    }
  }
}

#[test]
fn key_keeps_id() {
  let key = FlagKey::new("feature-enabled");

  assert_eq!(key.id(), "feature-enabled");
}

#[test]
fn keys_with_same_id_are_equal_across_types() {
  let flag = FlagKey::new("shared");
  let counter = GCounterKey::new("shared");
  let pn_counter = PNCounterKey::new("shared");
  let pn_counter_map = PNCounterMapKey::<String>::new("shared");

  assert_eq!(flag, counter);
  assert_eq!(counter, pn_counter);
  assert_eq!(pn_counter, pn_counter_map);
}

#[test]
fn key_hash_uses_id_only() {
  let flag = FlagKey::new("shared");
  let counter = GCounterKey::new("shared");

  let mut left = StableHasher::default();
  let mut right = StableHasher::default();
  flag.hash(&mut left);
  counter.hash(&mut right);

  assert_eq!(left.finish(), right.finish());
}

#[test]
fn concrete_key_aliases_are_constructible() {
  let _flag: FlagKey = Key::new("flag");
  let _g_counter: GCounterKey = Key::new("g-counter");
  let _pn_counter: PNCounterKey = Key::new("pn-counter");
  let _pn_counter_map: PNCounterMapKey<String> = Key::new("pn-counter-map");
  let _: BTreeMap<String, i128> = BTreeMap::new();
}
