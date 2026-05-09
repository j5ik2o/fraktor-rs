use core::marker::PhantomData;

use super::stream_graph::StreamGraph;
use crate::{
  StreamError,
  dsl::{Flow, Sink, Source, combine_mat},
  materialization::{KeepLeft, MatCombine, MatCombineRule, StreamNotUsed},
  shape::{Inlet, Outlet},
};

#[cfg(test)]
mod tests;

/// Builder facade for composing stream graphs.
///
/// Supports both linear composition (via [`via`](Self::via) / [`to`](Self::to))
/// and non-linear topologies (via [`add_source`](Self::add_source) /
/// [`add_flow`](Self::add_flow) / [`add_sink`](Self::add_sink) /
/// [`connect`](Self::connect)).
pub(crate) struct GraphDslBuilder<In, Out, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  /// Tracks `In`/`Out` at the type level without storing values.
  ///
  /// `fn(In) -> Out` makes the marker contravariant in `In` and covariant in
  /// `Out`, matching the builder's data-flow semantics.
  _pd:   PhantomData<fn(In) -> Out>,
}

impl<T> GraphDslBuilder<T, T, StreamNotUsed> {
  /// Creates an empty builder.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { graph: StreamGraph::new(), mat: StreamNotUsed::new(), _pd: PhantomData }
  }
}

impl<T> Default for GraphDslBuilder<T, T, StreamNotUsed> {
  fn default() -> Self {
    Self::new()
  }
}

impl<In, Out, Mat> GraphDslBuilder<In, Out, Mat> {
  /// Creates a builder from a pre-built stream graph.
  #[must_use]
  pub(crate) fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  /// Creates a builder from an existing flow.
  #[must_use]
  pub(crate) fn from_flow(flow: Flow<In, Out, Mat>) -> Self {
    let (graph, mat) = flow.into_parts();
    Self::from_graph(graph, mat)
  }

  /// Maps the materialized value while keeping the graph unchanged.
  #[must_use]
  pub(crate) fn map_materialized_value<Mat2, F>(self, func: F) -> GraphDslBuilder<In, Out, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    let (graph, mat) = self.into_parts();
    GraphDslBuilder::from_graph(graph, func(mat))
  }

  /// Consumes the builder and returns the underlying graph and materialized value.
  #[must_use]
  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
  }

  /// Finalizes the builder as a flow.
  #[must_use]
  pub(crate) fn build(self) -> Flow<In, Out, Mat> {
    Flow::from_graph(self.graph, self.mat)
  }

  /// Appends a flow to this builder.
  #[must_use]
  pub(crate) fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> GraphDslBuilder<In, T, Mat>
  where
    T: Send + Sync + 'static, {
    self.via_mat(flow, KeepLeft)
  }

  /// Appends a flow with a custom materialized value rule.
  #[must_use]
  pub(crate) fn via_mat<T, Mat2, C>(self, flow: Flow<Out, T, Mat2>, _combine: C) -> GraphDslBuilder<In, T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (flow_graph, right_mat) = flow.into_parts();
    graph.append(flow_graph);
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    GraphDslBuilder::from_graph(graph, mat)
  }

  /// Connects the builder to a sink.
  #[must_use]
  pub(crate) fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    self.build().to(sink)
  }

  /// Connects the builder to a sink with a custom materialized value rule.
  #[must_use]
  pub(crate) fn into_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, combine: C) -> Sink<In, C::Out>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.build().into_mat(sink, combine)
  }

  /// Imports a source graph and returns its outlet port.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sourceGraph)`.
  /// The source's materialized value is discarded.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has no outlet.
  pub(crate) fn add_source<T, Mat2>(&mut self, source: Source<T, Mat2>) -> Result<Outlet<T>, StreamError>
  where
    T: Send + Sync + 'static, {
    let (other_graph, _mat) = source.into_parts();
    let outlet_id = other_graph.tail_outlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok(Outlet::from_id(outlet_id))
  }

  /// Imports a flow graph and returns its (inlet, outlet) port pair.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(flowGraph)`.
  /// The flow's materialized value is discarded.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has no inlet or outlet.
  pub(crate) fn add_flow<I, O, Mat2>(&mut self, flow: Flow<I, O, Mat2>) -> Result<(Inlet<I>, Outlet<O>), StreamError>
  where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static, {
    let (other_graph, _mat) = flow.into_parts();
    let inlet_id = other_graph.head_inlet().ok_or(StreamError::InvalidConnection)?;
    let outlet_id = other_graph.tail_outlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok((Inlet::from_id(inlet_id), Outlet::from_id(outlet_id)))
  }

  /// Imports a sink graph and returns its inlet port.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sinkGraph)`.
  /// The sink's materialized value is discarded.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has no inlet.
  pub(crate) fn add_sink<T, Mat2>(&mut self, sink: Sink<T, Mat2>) -> Result<Inlet<T>, StreamError>
  where
    T: Send + Sync + 'static, {
    let (other_graph, _mat) = sink.into_parts();
    let inlet_id = other_graph.head_inlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok(Inlet::from_id(inlet_id))
  }

  /// Imports a source graph and returns its outlet port along with
  /// the materialized value.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sourceGraph)` with
  /// materialized value preservation.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has no outlet.
  pub(crate) fn add_source_mat<T, Mat2>(&mut self, source: Source<T, Mat2>) -> Result<(Outlet<T>, Mat2), StreamError>
  where
    T: Send + Sync + 'static, {
    let (other_graph, mat) = source.into_parts();
    let outlet_id = other_graph.tail_outlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok((Outlet::from_id(outlet_id), mat))
  }

  /// Imports a flow graph and returns its (inlet, outlet) port pair along
  /// with the materialized value.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(flowGraph)` with
  /// materialized value preservation.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has no inlet or outlet.
  pub(crate) fn add_flow_mat<I, O, Mat2>(
    &mut self,
    flow: Flow<I, O, Mat2>,
  ) -> Result<(Inlet<I>, Outlet<O>, Mat2), StreamError>
  where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static, {
    let (other_graph, mat) = flow.into_parts();
    let inlet_id = other_graph.head_inlet().ok_or(StreamError::InvalidConnection)?;
    let outlet_id = other_graph.tail_outlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok((Inlet::from_id(inlet_id), Outlet::from_id(outlet_id), mat))
  }

  /// Imports a sink graph and returns its inlet port along with the
  /// materialized value.
  ///
  /// Corresponds to Pekko's `GraphDSL.Builder.add(sinkGraph)` with
  /// materialized value preservation.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has no inlet.
  pub(crate) fn add_sink_mat<T, Mat2>(&mut self, sink: Sink<T, Mat2>) -> Result<(Inlet<T>, Mat2), StreamError>
  where
    T: Send + Sync + 'static, {
    let (other_graph, mat) = sink.into_parts();
    let inlet_id = other_graph.head_inlet().ok_or(StreamError::InvalidConnection)?;
    self.graph.append_unwired(other_graph);
    Ok((Inlet::from_id(inlet_id), mat))
  }

  /// Connects an outlet to an inlet within this builder's graph.
  ///
  /// Corresponds to Pekko's `~>` operator.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when a port is unknown.
  pub(crate) fn connect<T>(&mut self, from: &Outlet<T>, to: &Inlet<T>) -> Result<(), StreamError> {
    self.graph.connect(from, to, MatCombine::Left)
  }

  /// Connects an outlet through a flow to an inlet in one step.
  ///
  /// Equivalent to calling [`add_flow`](Self::add_flow) followed by two
  /// [`connect`](Self::connect) calls. The flow's materialized value is
  /// discarded.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has
  /// missing ports or the connections fail.
  pub(crate) fn connect_via<T, U, Mat2>(
    &mut self,
    from: &Outlet<T>,
    flow: Flow<T, U, Mat2>,
    to: &Inlet<U>,
  ) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static, {
    let (flow_in, flow_out) = self.add_flow(flow)?;
    self.connect(from, &flow_in)?;
    self.connect(&flow_out, to)?;
    Ok(())
  }

  /// Connects an outlet through a flow, returning the flow's outlet.
  ///
  /// Equivalent to [`add_flow`](Self::add_flow) + [`connect`](Self::connect),
  /// but returns the downstream outlet for further chaining.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the flow graph has
  /// missing ports or the connection fails.
  pub(crate) fn wire_via<T, U, Mat2>(
    &mut self,
    from: &Outlet<T>,
    flow: Flow<T, U, Mat2>,
  ) -> Result<Outlet<U>, StreamError>
  where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static, {
    let (inlet, outlet) = self.add_flow(flow)?;
    self.connect(from, &inlet)?;
    Ok(outlet)
  }

  /// Connects an outlet to a sink.
  ///
  /// Equivalent to [`add_sink`](Self::add_sink) + [`connect`](Self::connect).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the sink graph has
  /// no inlet or the connection fails.
  pub(crate) fn wire_to<T, Mat2>(&mut self, from: &Outlet<T>, sink: Sink<T, Mat2>) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static, {
    let inlet = self.add_sink(sink)?;
    self.connect(from, &inlet)
  }

  /// Connects a source to an inlet.
  ///
  /// Equivalent to [`add_source`](Self::add_source) + [`connect`](Self::connect).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] if the source graph has
  /// no outlet or the connection fails.
  pub(crate) fn wire_from<T, Mat2>(&mut self, source: Source<T, Mat2>, to: &Inlet<T>) -> Result<(), StreamError>
  where
    T: Send + Sync + 'static, {
    let outlet = self.add_source(source)?;
    self.connect(&outlet, to)
  }
}
