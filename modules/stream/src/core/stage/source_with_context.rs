use core::marker::PhantomData;

use super::{flow_with_context::FlowWithContext, source::Source};

#[cfg(test)]
mod tests;

/// Context-preserving source wrapper.
///
/// Wraps an inner `Source<(Ctx, Out), Mat>` and automatically propagates
/// the context value through stream operators.
pub struct SourceWithContext<Ctx, Out, Mat> {
  inner: Source<(Ctx, Out), Mat>,
  _pd:   PhantomData<fn() -> Ctx>,
}

impl<Ctx, Out, Mat> SourceWithContext<Ctx, Out, Mat>
where
  Ctx: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Creates a context-preserving source from an inner tuple source.
  #[must_use]
  pub fn from_source(inner: Source<(Ctx, Out), Mat>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Returns the inner tuple source.
  #[must_use]
  pub fn as_source(self) -> Source<(Ctx, Out), Mat> {
    self.inner
  }

  /// Maps the output value while preserving context.
  #[must_use]
  pub fn map<T, F>(self, mut func: F) -> SourceWithContext<Ctx, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    let mapped = self.inner.map(move |(ctx, value)| (ctx, func(value)));
    SourceWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Filters elements by value while preserving context.
  #[must_use]
  pub fn filter<F>(self, mut predicate: F) -> SourceWithContext<Ctx, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let filtered = self.inner.filter(move |(_, value)| predicate(value));
    SourceWithContext { inner: filtered, _pd: PhantomData }
  }

  /// Maps the context value while preserving the output.
  #[must_use]
  pub fn map_context<Ctx2, F>(self, mut func: F) -> SourceWithContext<Ctx2, Out, Mat>
  where
    Ctx2: Send + Sync + 'static,
    F: FnMut(Ctx) -> Ctx2 + Send + Sync + 'static, {
    let mapped = self.inner.map(move |(ctx, value)| (func(ctx), value));
    SourceWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Composes with a context-preserving flow.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: FlowWithContext<Ctx, Out, T, Mat2>) -> SourceWithContext<Ctx, T, Mat>
  where
    T: Send + Sync + 'static, {
    let composed = self.inner.via(flow.as_flow());
    SourceWithContext { inner: composed, _pd: PhantomData }
  }
}
