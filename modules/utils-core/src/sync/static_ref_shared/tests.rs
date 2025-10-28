#![allow(clippy::disallowed_types)]

use super::StaticRefShared;
use crate::sync::shared::SharedDyn;

static VALUE: u32 = 42;

#[test]
fn deref_returns_inner_reference() {
  let shared = StaticRefShared::new(&VALUE);
  assert_eq!(*shared, 42);
}

#[test]
fn clone_preserves_identity() {
  let shared = StaticRefShared::new(&VALUE);
  let cloned = shared;
  assert!(core::ptr::eq(shared.as_ref(), cloned.as_ref()));
}

static OTHER: (u32, u32) = (1, 2);

#[test]
fn into_dyn_maps_to_trait_view() {
  trait Pair {
    fn left(&self) -> u32;
  }

  impl Pair for (u32, u32) {
    fn left(&self) -> u32 {
      self.0
    }
  }

  let shared = StaticRefShared::new(&OTHER);
  let dyn_shared = shared.into_dyn(|pair| pair as &dyn Pair);
  assert_eq!(dyn_shared.left(), 1);
}
