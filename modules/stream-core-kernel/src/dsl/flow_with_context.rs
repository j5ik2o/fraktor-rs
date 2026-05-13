use alloc::vec::Vec;
use core::{future::Future, marker::PhantomData};

use super::{
  KeepLeft, KeepRight, MatCombineRule, StreamDslError, StreamNotUsed, ThrottleMode, extract_last_ctx_and_values,
  flow::Flow, sink::Sink,
};
use crate::r#impl::StreamError;

#[cfg(test)]
#[path = "flow_with_context_test.rs"]
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
  pub fn into_flow(self) -> Flow<(Ctx, In), (Ctx, Out), Mat> {
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
    let inner = remap_input.via_mat(self.inner, KeepRight).via(remap_output);
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Expands each element into multiple elements, each carrying the same context.
  #[must_use]
  pub fn map_concat<Out2, I, F>(self, mut f: F) -> FlowWithContext<Ctx, In, Out2, Mat>
  where
    Ctx: Clone,
    Out2: Send + Sync + 'static,
    I: IntoIterator<Item = Out2> + 'static,
    F: FnMut(Out) -> I + Send + Sync + 'static, {
    let mapped = self.inner.map_concat(move |(ctx, value)| f(value).into_iter().map(move |item| (ctx.clone(), item)));
    FlowWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Filters elements where the predicate returns false, preserving context.
  #[must_use]
  pub fn filter_not<F>(self, mut predicate: F) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(move |value| !predicate(value))
  }

  /// Filters and maps elements in one step, preserving context.
  #[must_use]
  pub fn collect<Out2, F>(self, mut f: F) -> FlowWithContext<Ctx, In, Out2, Mat>
  where
    Out2: Send + Sync + 'static,
    F: FnMut(Out) -> Option<Out2> + Send + Sync + 'static, {
    let mapped = self.inner.map_concat(move |(ctx, value)| f(value).map(|out| (ctx, out)));
    FlowWithContext { inner: mapped, _pd: PhantomData }
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
  ) -> Result<FlowWithContext<Ctx, In, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static, {
    let mapped = self.inner.map_async(parallelism, move |(ctx, value)| {
      let fut = f(value);
      async move { (ctx, fut.await) }
    })?;
    Ok(FlowWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Groups elements into fixed-size batches.
  ///
  /// The context of each group is the context of the last element in the group.
  /// Core logic is shared via [`extract_last_ctx_and_values`]; the wrapper type
  /// differs between `FlowWithContext` and `SourceWithContext`.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `size` is zero.
  pub fn grouped(self, size: usize) -> Result<FlowWithContext<Ctx, In, Vec<Out>, Mat>, StreamDslError> {
    let grouped = self.inner.grouped(size)?;
    let mapped = grouped.map_concat(extract_last_ctx_and_values);
    Ok(FlowWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Creates sliding windows over elements.
  ///
  /// The context of each window is the context of the last element in the window.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `size` is zero.
  pub fn sliding(self, size: usize) -> Result<FlowWithContext<Ctx, In, Vec<Out>, Mat>, StreamDslError>
  where
    Ctx: Clone,
    Out: Clone, {
    let sliding = self.inner.sliding(size)?;
    let mapped = sliding.map_concat(extract_last_ctx_and_values);
    Ok(FlowWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Composes with another context-preserving flow.
  #[must_use]
  pub fn via<T, Mat2>(self, other: FlowWithContext<Ctx, Out, T, Mat2>) -> FlowWithContext<Ctx, In, T, Mat>
  where
    T: Send + Sync + 'static, {
    let composed = self.inner.via(other.inner);
    FlowWithContext { inner: composed, _pd: PhantomData }
  }

  /// Composes with another context-preserving flow using a custom materialized value rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(
    self,
    other: FlowWithContext<Ctx, Out, T, Mat2>,
    combine: C,
  ) -> FlowWithContext<Ctx, In, T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let composed = self.inner.via_mat(other.inner, combine);
    FlowWithContext { inner: composed, _pd: PhantomData }
  }

  /// Adds a side sink that receives only the data value.
  #[must_use]
  pub fn also_to<Mat2>(self, sink: Sink<Out, Mat2>) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let inner = self.inner.also_to(sink.contramap(|(_, value): (Ctx, Out)| value));
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Adds a side sink that receives only the context value.
  #[must_use]
  pub fn also_to_context<Mat2>(self, sink: Sink<Ctx, Mat2>) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let inner = self.inner.also_to(sink.contramap(|(ctx, _): (Ctx, Out)| ctx));
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Taps each data value to a side sink while preserving the main path.
  #[must_use]
  pub fn wire_tap<Mat2>(self, sink: Sink<Out, Mat2>) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let inner = self.inner.wire_tap_mat(sink.contramap(|(_, value): (Ctx, Out)| value), KeepLeft);
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Taps each context value to a side sink while preserving the main path.
  #[must_use]
  pub fn wire_tap_context<Mat2>(self, sink: Sink<Ctx, Mat2>) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let inner = self.inner.wire_tap_mat(sink.contramap(|(ctx, _): (Ctx, Out)| ctx), KeepLeft);
    FlowWithContext { inner, _pd: PhantomData }
  }

  /// Transforms upstream failures while preserving context.
  ///
  /// Normal elements pass through unchanged. When the stream fails, the
  /// mapper transforms the error before propagation.
  #[must_use]
  pub fn map_error<F>(self, mapper: F) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    let mapped = self.inner.map_error(mapper);
    FlowWithContext { inner: mapped, _pd: PhantomData }
  }

  /// Limits the rate of elements while preserving context.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `capacity` is zero.
  pub fn throttle(
    self,
    capacity: usize,
    mode: ThrottleMode,
  ) -> Result<FlowWithContext<Ctx, In, Out, Mat>, StreamDslError> {
    let throttled = self.inner.throttle(capacity, mode)?;
    Ok(FlowWithContext { inner: throttled, _pd: PhantomData })
  }

  /// Maps elements asynchronously while serializing work per partition.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `parallelism` is zero.
  pub fn map_async_partitioned<Out2, P, Partitioner, F, Fut>(
    self,
    parallelism: usize,
    mut partitioner: Partitioner,
    mut func: F,
  ) -> Result<FlowWithContext<Ctx, In, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static, {
    let mapped = self.inner.map_async_partitioned(
      parallelism,
      move |(_, value)| partitioner(value),
      move |(ctx, value), partition| {
        let fut = func(value, partition);
        async move { (ctx, fut.await) }
      },
    )?;
    Ok(FlowWithContext { inner: mapped, _pd: PhantomData })
  }

  /// Maps elements asynchronously per partition without preserving downstream order.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `parallelism` is zero.
  pub fn map_async_partitioned_unordered<Out2, P, Partitioner, F, Fut>(
    self,
    parallelism: usize,
    mut partitioner: Partitioner,
    mut func: F,
  ) -> Result<FlowWithContext<Ctx, In, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static, {
    let mapped = self.inner.map_async_partitioned_unordered(
      parallelism,
      move |(_, value)| partitioner(value),
      move |(ctx, value), partition| {
        let fut = func(value, partition);
        async move { (ctx, fut.await) }
      },
    )?;
    Ok(FlowWithContext { inner: mapped, _pd: PhantomData })
  }
}
