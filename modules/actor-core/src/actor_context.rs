//! Actor execution context placeholder.

use core::marker::PhantomData;

/// Provides contextual operations for an actor while processing a message.
pub struct ActorContext<'a> {
  _marker: PhantomData<&'a ()>,
}
