use core::marker::PhantomData;

use super::{StreamNotUsed, flow::Flow};

#[cfg(test)]
mod tests;

/// Context-preserving flow wrapper.
///
/// Wraps an inner `Flow<(Ctx, In), (Ctx, Out), Mat>` and automatically
/// propagates the context value through stream operators.
pub struct FlowWithContext<Ctx, In, Out, Mat> {
  inner: Flow<(Ctx, In), (Ctx, Out), Mat>,
  _pd:   PhantomData<fn(Ctx)>,
}

impl<Ctx, In, Out, Mat> FlowWithContext<Ctx, In, Out, Mat>
where
  Ctx: Send + Sync + 'static,
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Creates a context-preserving flow from an inner tuple flow.
  #[must_use]
  pub fn from_flow(inner: Flow<(Ctx, In), (Ctx, Out), Mat>) -> Self {
    Self { inner, _pd: PhantomData }
  }

  /// Returns the inner tuple flow.
  #[must_use]
  pub fn as_flow(self) -> Flow<(Ctx, In), (Ctx, Out), Mat> {
    self.inner
  }

  /// Maps the output value while preserving context.
  #[must_use]
  pub fn map<T, F>(self, mut func: F) -> FlowWithContext<Ctx, In, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    let mapped = self.inner.map(move |(ctx, value)| (ctx, func(value)));
    FlowWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Filters elements by value while preserving context.
  #[must_use]
  pub fn filter<F>(self, mut predicate: F) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let filtered = self.inner.filter(move |(_, value)| predicate(value));
    FlowWithContext { inner: filtered, _pd: PhantomData }
  }

  /// Remaps the context value on both input and output sides.
  ///
  /// Because a flow has both an input and an output side, remapping the
  /// context requires a pair of functions: `forward` converts `Ctx` to
  /// `Ctx2` on the output, while `reverse` converts `Ctx2` back to `Ctx`
  /// on the input.
  #[must_use]
  pub fn map_context<Ctx2, F, G>(self, forward: F, reverse: G) -> FlowWithContext<Ctx2, In, Out, Mat>
  where
    Ctx2: Send + Sync + 'static,
    F: Fn(Ctx) -> Ctx2 + Send + Sync + 'static,
    G: Fn(Ctx2) -> Ctx + Send + Sync + 'static, {
    let remap_input: Flow<(Ctx2, In), (Ctx, In), StreamNotUsed> =
      Flow::from_function(move |(ctx2, value)| (reverse(ctx2), value));
    let remap_output: Flow<(Ctx, Out), (Ctx2, Out), StreamNotUsed> =
      Flow::from_function(move |(ctx, value)| (forward(ctx), value));
    let inner = remap_input.via_mat(self.inner, super::keep_right::KeepRight).via(remap_output);
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Composes with another context-preserving flow.
  #[must_use]
  pub fn via<T, Mat2>(self, other: FlowWithContext<Ctx, Out, T, Mat2>) -> FlowWithContext<Ctx, In, T, Mat>
  where
    T: Send + Sync + 'static, {
    let composed = self.inner.via(other.inner);
    FlowWithContext { inner: composed, _pd: PhantomData }
  }
}
