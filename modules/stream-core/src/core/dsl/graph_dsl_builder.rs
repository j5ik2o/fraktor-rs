use super::{
  Flow, MatCombineRule, Sink, Source, StreamError, StreamGraph, StreamNotUsed,
  shape::{Inlet, Outlet},
};
use crate::core::r#impl::GraphDslBuilder as InternalGraphDslBuilder;

#[cfg(test)]
mod tests;

/// Builder for composing stream graphs.
pub struct GraphDslBuilder<In, Out, Mat> {
  inner: InternalGraphDslBuilder<In, Out, Mat>,
}

impl<T> GraphDslBuilder<T, T, StreamNotUsed> {
  /// Creates an empty builder.
  #[must_use]
  pub fn new() -> Self {
    Self::from_inner(InternalGraphDslBuilder::new())
  }
}

impl<T> Default for GraphDslBuilder<T, T, StreamNotUsed> {
  fn default() -> Self {
    Self::new()
  }
}

impl<In, Out, Mat> GraphDslBuilder<In, Out, Mat> {
  pub(in crate::core) fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self::from_inner(InternalGraphDslBuilder::from_graph(graph, mat))
  }

  const fn from_inner(inner: InternalGraphDslBuilder<In, Out, Mat>) -> Self {
    Self { inner }
  }

  fn into_parts(self) -> (StreamGraph, Mat) {
    self.inner.into_parts()
  }

  pub(super) fn into_source(self) -> Source<Out, Mat>
  where
    Out: Send + Sync + 'static, {
    let (graph, mat) = self.into_parts();
    Source::from_graph(graph, mat)
  }

  pub(super) fn into_sink(self) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static, {
    let (graph, mat) = self.into_parts();
    Sink::from_graph(graph, mat)
  }

  /// Creates a builder from an existing flow.
  #[must_use]
  pub fn from_flow(flow: Flow<In, Out, Mat>) -> Self {
    Self::from_inner(InternalGraphDslBuilder::from_flow(flow))
  }

  /// Maps the materialized value while keeping the graph unchanged.
  #[must_use]
  pub fn map_materialized_value<Mat2, F>(self, func: F) -> GraphDslBuilder<In, Out, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    GraphDslBuilder::from_inner(self.inner.map_materialized_value(func))
  }

  /// Finalizes the builder as a flow.
  #[must_use]
  pub fn build(self) -> Flow<In, Out, Mat> {
    self.inner.build()
  }

  /// Appends a flow to this builder.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> GraphDslBuilder<In, T, Mat>
  where
    T: Send + Sync + 'static, {
    GraphDslBuilder::from_inner(self.inner.via(flow))
  }

  /// Appends a flow with a custom materialized value rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(self, flow: Flow<Out, T, Mat2>, combine: C) -> GraphDslBuilder<In, T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    GraphDslBuilder::from_inner(self.inner.via_mat(flow, combine))
  }

  /// Connects the builder to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    self.inner.to(sink)
  }

  /// Connects the builder to a sink with a custom materialized value rule.
  #[must_use]
  pub fn into_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, combine: C) -> Sink<In, C::Out>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.inner.into_mat(sink, combine)
  }

  /// Imports a source graph and returns its outlet port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has no outlet.
  pub fn add_source<T, Mat2>(&mut self, source: Source<T, Mat2>) -> Result<Outlet<T>, StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.add_source(source)
  }

  /// Imports a flow graph and returns its inlet and outlet ports.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has no inlet or outlet.
  pub fn add_flow<I, O, Mat2>(&mut self, flow: Flow<I, O, Mat2>) -> Result<(Inlet<I>, Outlet<O>), StreamError>
  where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static, {
    self.inner.add_flow(flow)
  }

  /// Imports a sink graph and returns its inlet port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has no inlet.
  pub fn add_sink<T, Mat2>(&mut self, sink: Sink<T, Mat2>) -> Result<Inlet<T>, StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.add_sink(sink)
  }

  /// Imports a source graph and returns its outlet port and materialized value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has no outlet.
  pub fn add_source_mat<T, Mat2>(&mut self, source: Source<T, Mat2>) -> Result<(Outlet<T>, Mat2), StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.add_source_mat(source)
  }

  /// Imports a flow graph and returns its ports and materialized value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has no inlet or outlet.
  pub fn add_flow_mat<I, O, Mat2>(
    &mut self,
    flow: Flow<I, O, Mat2>,
  ) -> Result<(Inlet<I>, Outlet<O>, Mat2), StreamError>
  where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static, {
    self.inner.add_flow_mat(flow)
  }

  /// Imports a sink graph and returns its inlet port and materialized value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has no inlet.
  pub fn add_sink_mat<T, Mat2>(&mut self, sink: Sink<T, Mat2>) -> Result<(Inlet<T>, Mat2), StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.add_sink_mat(sink)
  }

  /// Connects an outlet to an inlet within this builder's graph.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when a port is unknown.
  pub fn connect<T>(&mut self, from: &Outlet<T>, to: &Inlet<T>) -> Result<(), StreamError> {
    self.inner.connect(from, to)
  }

  /// Connects an outlet through a flow to an inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has missing ports or a connection
  /// fails.
  pub fn connect_via<T, U, Mat2>(
    &mut self,
    from: &Outlet<T>,
    flow: Flow<T, U, Mat2>,
    to: &Inlet<U>,
  ) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static, {
    self.inner.connect_via(from, flow, to)
  }

  /// Connects an outlet through a flow and returns the downstream outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has missing ports or a connection
  /// fails.
  pub fn wire_via<T, U, Mat2>(&mut self, from: &Outlet<T>, flow: Flow<T, U, Mat2>) -> Result<Outlet<U>, StreamError>
  where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static, {
    self.inner.wire_via(from, flow)
  }

  /// Connects an outlet to a sink.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has no inlet or the connection
  /// fails.
  pub fn wire_to<T, Mat2>(&mut self, from: &Outlet<T>, sink: Sink<T, Mat2>) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.wire_to(from, sink)
  }

  /// Connects a source to an inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has no outlet or the connection
  /// fails.
  pub fn wire_from<T, Mat2>(&mut self, source: Source<T, Mat2>, to: &Inlet<T>) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static, {
    self.inner.wire_from(source, to)
  }
}
