use alloc::vec::Vec;
use core::{future::Future, marker::PhantomData};

use super::{StreamDslError, extract_last_ctx_and_values, flow_with_context::FlowWithContext, source::Source};

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

  /// Expands each element into multiple elements, each carrying the same context.
  #[must_use]
  pub fn map_concat<Out2, I, F>(self, mut f: F) -> SourceWithContext<Ctx, Out2, Mat>
  where
    Ctx: Clone,
    Out2: Send + Sync + 'static,
    I: IntoIterator<Item = Out2> + 'static,
    F: FnMut(Out) -> I + Send + Sync + 'static, {
    let mapped = self.inner.map_concat(move |(ctx, value)| f(value).into_iter().map(move |item| (ctx.clone(), item)));
    SourceWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Filters elements where the predicate returns false, preserving context.
  #[must_use]
  pub fn filter_not<F>(self, mut predicate: F) -> SourceWithContext<Ctx, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(move |value| !predicate(value))
  }

  /// Filters and maps elements in one step, preserving context.
  #[must_use]
  pub fn collect<Out2, F>(self, mut f: F) -> SourceWithContext<Ctx, Out2, Mat>
  where
    Out2: Send + Sync + 'static,
    F: FnMut(Out) -> Option<Out2> + Send + Sync + 'static, {
    let mapped = self.inner.map_concat(move |(ctx, value)| f(value).map(|out| (ctx, out)));
    SourceWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Applies an async transformation to each element, preserving context.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `parallelism` is zero.
  pub fn map_async<Out2, Fut, F>(
    self,
    parallelism: usize,
    mut f: F,
  ) -> Result<SourceWithContext<Ctx, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static, {
    let mapped = self.inner.map_async(parallelism, move |(ctx, value)| {
      let fut = f(value);
      async move { (ctx, fut.await) }
    })?;
    Ok(SourceWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Groups elements into fixed-size batches.
  ///
  /// The context of each group is the context of the last element in the group.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `size` is zero.
  pub fn grouped(self, size: usize) -> Result<SourceWithContext<Ctx, Vec<Out>, Mat>, StreamDslError>
  where
    Ctx: Clone, {
    let grouped = self.inner.grouped(size)?;
    let mapped = grouped.map_concat(extract_last_ctx_and_values);
    Ok(SourceWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Creates sliding windows over elements.
  ///
  /// The context of each window is the context of the last element in the window.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `size` is zero.
  pub fn sliding(self, size: usize) -> Result<SourceWithContext<Ctx, Vec<Out>, Mat>, StreamDslError>
  where
    Ctx: Clone,
    Out: Clone, {
    let sliding = self.inner.sliding(size)?;
    let mapped = sliding.map_concat(extract_last_ctx_and_values);
    Ok(SourceWithContext { inner: mapped, _pd: PhantomData })
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
