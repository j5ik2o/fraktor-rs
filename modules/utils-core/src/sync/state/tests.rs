#![allow(clippy::disallowed_types)]

extern crate alloc;

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};

use super::*;

struct RcState<T>(Rc<RefCell<T>>);

impl<T> Clone for RcState<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> StateCell<T> for RcState<T> {
  type Ref<'a>
    = Ref<'a, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = RefMut<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self {
    Self(Rc::new(RefCell::new(value)))
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.0.borrow()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.0.borrow_mut()
  }
}

#[test]
fn with_ref_reads_current_value() {
  let cell = RcState::new(5_u32);
  let value = cell.with_ref(|v| *v);
  assert_eq!(value, 5);
}

#[test]
fn with_ref_mut_updates_value() {
  let cell = RcState::new(1_u32);
  cell.with_ref_mut(|v| *v = 10);
  assert_eq!(cell.with_ref(|v| *v), 10);
}
