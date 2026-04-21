use core::hash::{Hash, Hasher};
use std::{collections::hash_map::DefaultHasher, string::String};

use fraktor_actor_core_rs::core::kernel::util::ByteString;

#[test]
fn empty_creates_zero_length() {
  let bs = ByteString::empty();
  assert!(bs.is_empty());
  assert_eq!(bs.len(), 0);
  assert_eq!(bs.as_slice(), &[] as &[u8]);
}

#[test]
fn default_is_empty() {
  let bs = ByteString::default();
  assert!(bs.is_empty());
}

#[test]
fn from_slice_copies_data() {
  let data = [1u8, 2, 3, 4];
  let bs = ByteString::from_slice(&data);
  assert_eq!(bs.len(), 4);
  assert_eq!(bs.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn from_vec_takes_ownership() {
  let v = vec![10u8, 20, 30];
  let bs = ByteString::from_vec(v);
  assert_eq!(bs.len(), 3);
  assert_eq!(bs.as_slice(), &[10, 20, 30]);
}

#[test]
fn from_string_encodes_utf8() {
  let bs = ByteString::from_string("hello");
  assert_eq!(bs.len(), 5);
  assert_eq!(bs.as_slice(), b"hello");
}

#[test]
fn get_returns_byte_at_index() {
  let bs = ByteString::from_slice(&[10, 20, 30]);
  assert_eq!(bs.get(0), Some(10));
  assert_eq!(bs.get(2), Some(30));
  assert_eq!(bs.get(3), None);
}

#[test]
fn slice_creates_zero_copy_view() {
  let bs = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let sub = bs.slice(1, 4);
  assert_eq!(sub.len(), 3);
  assert_eq!(sub.as_slice(), &[2, 3, 4]);
  // ポインタがオリジナルのオフセット位置を指していることを検証（zero-copy）
  assert_eq!(sub.as_ptr(), unsafe { bs.as_ptr().add(1) });
}

#[test]
fn slice_clamps_to_valid_bounds() {
  let bs = ByteString::from_slice(&[1, 2, 3]);
  let sub = bs.slice(0, 100);
  assert_eq!(sub.as_slice(), &[1, 2, 3]);

  let sub2 = bs.slice(5, 10);
  assert!(sub2.is_empty());
}

#[test]
fn slice_of_slice_works() {
  let bs = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let s1 = bs.slice(1, 4);
  let s2 = s1.slice(1, 2);
  assert_eq!(s2.as_slice(), &[3]);
}

#[test]
fn take_returns_first_n_bytes() {
  let bs = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let head = bs.take(3);
  assert_eq!(head.as_slice(), &[1, 2, 3]);
}

#[test]
fn drop_prefix_skips_first_n_bytes() {
  let bs = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let tail = bs.drop_prefix(2);
  assert_eq!(tail.as_slice(), &[3, 4, 5]);
}

#[test]
fn concat_joins_two_byte_strings() {
  let a = ByteString::from_slice(&[1, 2]);
  let b = ByteString::from_slice(&[3, 4]);
  let combined = a.concat(&b);
  assert_eq!(combined.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn concat_with_empty_returns_other() {
  let a = ByteString::from_slice(&[1, 2]);
  let empty = ByteString::empty();

  let r1 = a.concat(&empty);
  assert_eq!(r1.as_slice(), &[1, 2]);

  let r2 = empty.concat(&a);
  assert_eq!(r2.as_slice(), &[1, 2]);
}

#[test]
fn to_vec_copies_contents() {
  let bs = ByteString::from_slice(&[1, 2, 3]);
  let v = bs.to_vec();
  assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn decode_string_valid_utf8() {
  let bs = ByteString::from_string("日本語");
  let s = bs.decode_string().expect("valid UTF-8");
  assert_eq!(s, "日本語");
}

#[test]
fn decode_string_invalid_utf8_returns_err() {
  let bs = ByteString::from_slice(&[0xFF, 0xFE]);
  assert!(bs.decode_string().is_err());
}

#[test]
fn equality_compares_by_content() {
  let a = ByteString::from_slice(&[1, 2, 3]);
  let b = ByteString::from_slice(&[1, 2, 3]);
  let c = ByteString::from_slice(&[4, 5, 6]);
  assert_eq!(a, b);
  assert_ne!(a, c);
}

#[test]
fn sliced_equality() {
  let full = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let sliced = full.slice(0, 3);
  let direct = ByteString::from_slice(&[1, 2, 3]);
  assert_eq!(sliced, direct);
}

#[test]
fn clone_shares_data() {
  let original = ByteString::from_slice(&[1, 2, 3]);
  let cloned = original.clone();
  assert_eq!(original, cloned);
  assert_eq!(original.as_slice(), cloned.as_slice());
}

#[test]
fn starts_with_checks_prefix() {
  let bs = ByteString::from_slice(&[1, 2, 3, 4]);
  assert!(bs.starts_with(&[1, 2]));
  assert!(!bs.starts_with(&[2, 3]));
  assert!(bs.starts_with(&[]));
}

#[test]
fn index_of_finds_byte() {
  let bs = ByteString::from_slice(&[10, 20, 30, 40]);
  assert_eq!(bs.index_of(20), Some(1));
  assert_eq!(bs.index_of(99), None);
}

#[test]
fn from_trait_impls() {
  let from_slice: ByteString = [1u8, 2, 3].as_slice().into();
  assert_eq!(from_slice.as_slice(), &[1, 2, 3]);

  let from_vec: ByteString = vec![4u8, 5, 6].into();
  assert_eq!(from_vec.as_slice(), &[4, 5, 6]);

  let from_str: ByteString = "abc".into();
  assert_eq!(from_str.as_slice(), b"abc");

  let from_string: ByteString = String::from("xyz").into();
  assert_eq!(from_string.as_slice(), b"xyz");
}

#[test]
fn as_ref_returns_slice() {
  let bs = ByteString::from_slice(&[1, 2, 3]);
  let r: &[u8] = bs.as_ref();
  assert_eq!(r, &[1, 2, 3]);
}

#[test]
fn debug_displays_len() {
  let bs = ByteString::from_slice(&[1, 2, 3]);
  let dbg = format!("{:?}", bs);
  assert!(dbg.contains("ByteString"));
  assert!(dbg.contains("3"));
}

#[test]
fn hash_is_content_based() {
  fn compute_hash(bs: &ByteString) -> u64 {
    let mut hasher = DefaultHasher::new();
    bs.hash(&mut hasher);
    hasher.finish()
  }

  let a = ByteString::from_slice(&[1, 2, 3]);
  let b = ByteString::from_slice(&[1, 2, 3]);
  assert_eq!(compute_hash(&a), compute_hash(&b));
}
