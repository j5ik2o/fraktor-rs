extern crate std;

use crate::core::{actor::Pid, typed::actor::TypedActorContext as CoreTypedActorContext};

/// Read-only typed actor context wrapper for the standard runtime.
pub struct TypedActorContextRef<'ctx, 'inner, M>
where
  M: Send + Sync + 'static, {
  inner: &'ctx CoreTypedActorContext<'inner, M>,
}

impl<'ctx, 'inner, M> TypedActorContextRef<'ctx, 'inner, M>
where
  M: Send + Sync + 'static,
{
  /// Builds a read-only std-facing typed context wrapper from the core context.
  #[must_use]
  pub const fn from_core(core: &'ctx CoreTypedActorContext<'inner, M>) -> Self {
    Self { inner: core }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.inner.pid()
  }
}
