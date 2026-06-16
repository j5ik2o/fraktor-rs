use alloc::{collections::BTreeMap, string::String};
use core::hash::{Hash, Hasher};

use crate::ddata::{FlagKey, GCounterKey, Key, LWWRegisterKey, PNCounterKey, PNCounterMapKey, VersionVectorKey};

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
fn keys_with_same_id_are_equal_within_type() {
  let left = FlagKey::new("shared");
  let right = FlagKey::new("shared");

  assert_eq!(left, right);
}

#[test]
fn key_hash_uses_id_only() {
  let left_key = FlagKey::new("shared");
  let right_key = FlagKey::new("shared");

  let mut left_hash = StableHasher::default();
  let mut right_hash = StableHasher::default();
  left_key.hash(&mut left_hash);
  right_key.hash(&mut right_hash);

  assert_eq!(left_hash.finish(), right_hash.finish());
}

#[test]
fn concrete_key_aliases_are_constructible() {
  let _flag: FlagKey = Key::new("flag");
  let _g_counter: GCounterKey = Key::new("g-counter");
  let _pn_counter: PNCounterKey = Key::new("pn-counter");
  let _pn_counter_map: PNCounterMapKey<String> = Key::new("pn-counter-map");
  let _lww_register: LWWRegisterKey<String> = Key::new("lww-register");
  let _version_vector: VersionVectorKey = Key::new("version-vector");
  let _: BTreeMap<String, i128> = BTreeMap::new();
}
