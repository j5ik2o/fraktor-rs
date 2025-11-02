//! Borrowed message view placeholder.

use core::marker::PhantomData;

/// Represents a borrowed view of an actor message.
pub struct AnyMessageView<'a> {
  _marker: PhantomData<&'a ()>,
}
