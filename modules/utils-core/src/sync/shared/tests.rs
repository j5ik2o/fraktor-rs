#![allow(clippy::disallowed_types)]
extern crate alloc;

#[allow(clippy::disallowed_types)]
use alloc::rc::Rc;
use core::{cell::RefCell, ops::Deref};

use super::*;

#[allow(clippy::disallowed_types)]
#[derive(Clone, Debug)]
struct RcSharedCell(Rc<RefCell<u32>>);

impl RcSharedCell {
  fn new(value: u32) -> Self {
    Self(Rc::new(RefCell::new(value)))
  }
}

impl Deref for RcSharedCell {
  type Target = RefCell<u32>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl Shared<RefCell<u32>> for RcSharedCell {}

#[test]
fn default_try_unwrap_returns_err() {
  let shared = RcSharedCell::new(10);
  let result = shared.clone().try_unwrap();
  assert!(result.is_err(), "default try_unwrap should return Err");
}

#[test]
fn with_ref_exposes_inner_value() {
  let shared = RcSharedCell::new(7);
  let value = shared.with_ref(|cell| *cell.borrow());
  assert_eq!(value, 7);
}
