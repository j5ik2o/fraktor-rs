use alloc::boxed::Box;
use core::any::Any;

use crate::core::attributes::{Attribute, LogLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputBufferAttr {
  initial: usize,
  max:     usize,
}

impl Attribute for InputBufferAttr {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(self.clone())
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>().map_or(false, |candidate| self == candidate)
  }
}

#[test]
fn attribute_trait_object_can_be_downcast() {
  let attr = InputBufferAttr { initial: 16, max: 64 };
  let boxed: Box<dyn Attribute> = Box::new(attr.clone());
  let downcast = boxed.as_any().downcast_ref::<InputBufferAttr>();
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap(), &attr);
}

#[test]
fn different_attribute_types_are_distinguishable() {
  #[derive(Debug, Clone)]
  struct OtherAttr;

  impl Attribute for OtherAttr {
    fn as_any(&self) -> &dyn Any {
      self
    }

    fn clone_box(&self) -> Box<dyn Attribute> {
      Box::new(self.clone())
    }

    fn eq_attr(&self, _other: &dyn Any) -> bool {
      false
    }
  }

  let boxed: Box<dyn Attribute> = Box::new(InputBufferAttr { initial: 8, max: 32 });
  assert!(boxed.as_any().downcast_ref::<OtherAttr>().is_none());
}

#[test]
fn log_level_implements_attribute() {
  let boxed: Box<dyn Attribute> = Box::new(LogLevel::Info);
  let downcast = boxed.as_any().downcast_ref::<LogLevel>();
  assert!(downcast.is_some());
  assert_eq!(*downcast.unwrap(), LogLevel::Info);
}
