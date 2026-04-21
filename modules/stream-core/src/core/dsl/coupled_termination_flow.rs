use super::{Flow, Sink, Source};
use crate::core::materialization::{MatCombineRule, StreamNotUsed};

/// Factory for flows that propagate termination bidirectionally between the wrapped sink and
/// source.
///
/// Mirrors Apache Pekko's `object CoupledTerminationFlow`. Completion or cancellation on either
/// side is propagated to the other via a shared kill switch so both sides terminate together.
pub struct CoupledTerminationFlow;

impl CoupledTerminationFlow {
  /// Creates a coupled-termination flow from the given sink and source.
  ///
  /// Materialized values of the sink and source are discarded; the resulting flow materializes to
  /// [`StreamNotUsed`]. See [`Flow::from_sink_and_source_coupled`] for details on termination
  /// semantics.
  #[must_use]
  pub fn from_sink_and_source<In, Out, Mat1, Mat2>(
    sink: Sink<In, Mat1>,
    source: Source<Out, Mat2>,
  ) -> Flow<In, Out, StreamNotUsed>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    Flow::<In, Out, StreamNotUsed>::from_sink_and_source_coupled(sink, source)
  }

  /// Creates a coupled-termination flow from the given sink and source, combining their
  /// materialized values through `combine`.
  ///
  /// See [`Flow::from_sink_and_source_coupled_mat`] for details on termination semantics.
  #[must_use]
  pub fn from_sink_and_source_mat<In, Out, Mat1, Mat2, C>(
    sink: Sink<In, Mat1>,
    source: Source<Out, Mat2>,
    combine: C,
  ) -> Flow<In, Out, C::Out>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    C: MatCombineRule<Mat1, Mat2>, {
    Flow::<In, Out, StreamNotUsed>::from_sink_and_source_coupled_mat(sink, source, combine)
  }
}
