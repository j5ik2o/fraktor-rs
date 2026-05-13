use alloc::vec::Vec;
use core::{future::Future, marker::PhantomData};

use super::{
  MatCombineRule, StreamDslError, StreamNotUsed, ThrottleMode, extract_last_ctx_and_values, flow::Flow,
  flow_with_context::FlowWithContext, sink::Sink, source::Source,
};
use crate::r#impl::StreamError;

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
  pub fn into_source(self) -> Source<(Ctx, Out), Mat> {
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
  /// Core logic is shared via [`extract_last_ctx_and_values`]; the wrapper type
  /// differs between `FlowWithContext` and `SourceWithContext`.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `size` is zero.
  pub fn grouped(self, size: usize) -> Result<SourceWithContext<Ctx, Vec<Out>, Mat>, StreamDslError> {
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
    let composed = self.inner.via(flow.into_flow());
    SourceWithContext { inner: composed, _pd: PhantomData }
  }

  /// Composes with a context-preserving flow using a custom materialized value rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(
    self,
    flow: FlowWithContext<Ctx, Out, T, Mat2>,
    combine: C,
  ) -> SourceWithContext<Ctx, T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let composed = self.inner.via_mat(flow.into_flow(), combine);
    SourceWithContext { inner: composed, _pd: PhantomData }
  }

  /// Adds a side sink that receives only the data value.
  #[must_use]
  pub fn also_to<Mat2>(self, sink: Sink<Out, Mat2>) -> SourceWithContext<Ctx, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().also_to(sink);
    self.via(flow)
  }

  /// Adds a side sink that receives only the context value.
  #[must_use]
  pub fn also_to_context<Mat2>(self, sink: Sink<Ctx, Mat2>) -> SourceWithContext<Ctx, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().also_to_context(sink);
    self.via(flow)
  }

  /// Taps each data value to a side sink while preserving the main path.
  #[must_use]
  pub fn wire_tap<Mat2>(self, sink: Sink<Out, Mat2>) -> SourceWithContext<Ctx, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().wire_tap(sink);
    self.via(flow)
  }

  /// Taps each context value to a side sink while preserving the main path.
  #[must_use]
  pub fn wire_tap_context<Mat2>(self, sink: Sink<Ctx, Mat2>) -> SourceWithContext<Ctx, Out, Mat>
  where
    Ctx: Clone,
    Out: Clone, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().wire_tap_context(sink);
    self.via(flow)
  }

  /// Transforms upstream failures while preserving context.
  ///
  /// Normal elements pass through unchanged. When the stream fails, the
  /// mapper transforms the error before propagation.
  #[must_use]
  pub fn map_error<F>(self, mapper: F) -> SourceWithContext<Ctx, Out, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().map_error(mapper);
    self.via(flow)
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
  ) -> Result<SourceWithContext<Ctx, Out, Mat>, StreamDslError> {
    let flow = passthrough_flow_with_context::<Ctx, Out>().throttle(capacity, mode)?;
    Ok(self.via(flow))
  }

  /// Maps elements asynchronously while serializing work per partition.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `parallelism` is zero.
  pub fn map_async_partitioned<Out2, P, Partitioner, F, Fut>(
    self,
    parallelism: usize,
    partitioner: Partitioner,
    func: F,
  ) -> Result<SourceWithContext<Ctx, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static, {
    let flow = passthrough_flow_with_context::<Ctx, Out>().map_async_partitioned(parallelism, partitioner, func)?;
    Ok(self.via(flow))
  }

  /// Maps elements asynchronously per partition without preserving downstream order.
  ///
  /// # Errors
  ///
  /// Returns `StreamDslError` if `parallelism` is zero.
  pub fn map_async_partitioned_unordered<Out2, P, Partitioner, F, Fut>(
    self,
    parallelism: usize,
    partitioner: Partitioner,
    func: F,
  ) -> Result<SourceWithContext<Ctx, Out2, Mat>, StreamDslError>
  where
    Out2: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Out2> + Send + 'static, {
    let flow =
      passthrough_flow_with_context::<Ctx, Out>().map_async_partitioned_unordered(parallelism, partitioner, func)?;
    Ok(self.via(flow))
  }
}

fn passthrough_flow_with_context<Ctx, Out>() -> FlowWithContext<Ctx, Out, Out, StreamNotUsed>
where
  Ctx: Send + Sync + 'static,
  Out: Send + Sync + 'static, {
  FlowWithContext::from_flow(Flow::new().map(|value: (Ctx, Out)| value))
}
