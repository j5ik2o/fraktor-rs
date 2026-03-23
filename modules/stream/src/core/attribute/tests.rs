use core::any::Any;

use crate::core::{Attribute, LogLevel};

// --- InputBuffer as Attribute ---

/// Inline test attribute for verifying trait object behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InputBufferAttr {
  initial: usize,
  max:     usize,
}

impl Attribute for InputBufferAttr {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
    alloc::boxed::Box::new(self.clone())
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>().map_or(false, |o| self == o)
  }
}

#[test]
fn attribute_trait_object_can_be_downcast() {
  // Given: an attribute stored as a trait object
  let attr = InputBufferAttr { initial: 16, max: 64 };
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(attr.clone());

  // When: downcasting via as_any
  let downcast = boxed.as_any().downcast_ref::<InputBufferAttr>();

  // Then: the downcast succeeds and values match
  assert!(downcast.is_some());
  assert_eq!(downcast.unwrap(), &attr);
}

#[test]
fn different_attribute_types_are_distinguishable() {
  // Given: two different attribute types
  #[derive(Debug, Clone)]
  struct OtherAttr;
  impl Attribute for OtherAttr {
    fn as_any(&self) -> &dyn Any {
      self
    }

    fn clone_box(&self) -> alloc::boxed::Box<dyn Attribute> {
      alloc::boxed::Box::new(self.clone())
    }

    fn eq_attr(&self, _other: &dyn Any) -> bool {
      false
    }
  }

  let input_buffer = InputBufferAttr { initial: 8, max: 32 };
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(input_buffer);

  // When: attempting to downcast to the wrong type
  let wrong_cast = boxed.as_any().downcast_ref::<OtherAttr>();

  // Then: the downcast fails
  assert!(wrong_cast.is_none());
}

// --- LogLevel as Attribute ---

#[test]
fn log_level_implements_attribute() {
  // Given: a LogLevel wrapped as an Attribute trait object
  let level = LogLevel::Info;
  let boxed: alloc::boxed::Box<dyn Attribute> = alloc::boxed::Box::new(level);

  // When: downcasting
  let downcast = boxed.as_any().downcast_ref::<LogLevel>();

  // Then: the downcast succeeds
  assert!(downcast.is_some());
  assert_eq!(*downcast.unwrap(), LogLevel::Info);
}
