#[cfg(test)]
mod tests;

use crate::core::{
  StageDefinition,
  dsl::{Flow, FlowWithContext},
  graph::StreamGraph,
  stage::retry_flow_definition,
};

/// Retry flow factory for individual element retries with exponential backoff.
///
/// # Experimental
///
/// This corresponds to Pekko's `@ApiMayChange` `RetryFlow`. The API may change
/// in future releases as the upstream Pekko API stabilises.
///
/// # Overview
///
/// `RetryFlow` wraps an inner `Flow<In, Out, Mat>` and re-processes elements
/// whose output is deemed retryable by a caller-supplied `decide_retry`
/// function. On each retry the element is delayed by an exponentially
/// increasing backoff (with optional jitter) before being re-applied through
/// the inner flow.
pub struct RetryFlow;

impl RetryFlow {
  /// Creates a retry flow that re-processes elements on failure with
  /// exponential backoff.
  ///
  /// # Parameters
  ///
  /// * `min_backoff_ticks` – minimum delay (in ticks) before the first retry.
  /// * `max_backoff_ticks` – upper bound for the backoff delay.
  /// * `random_factor_permille` – jitter factor in permille (0–1000). `0` disables jitter.
  /// * `max_retries` – maximum number of retry attempts per element. After this limit the last
  ///   output is emitted downstream as-is.
  /// * `flow` – the inner flow whose output is checked for retries.
  /// * `decide_retry` – called with `(&input, &output)`. Returns `Some(retry_element)` to trigger a
  ///   retry, or `None` to accept the output.
  ///
  /// # Type constraints
  ///
  /// * `In: Clone` – elements must be cloneable so they can be re-sent on retry.
  /// * `Mat: Default` – the materialized value of the returned flow uses the default.
  #[must_use]
  pub fn with_backoff<In, Out, Mat, R>(
    min_backoff_ticks: u32,
    max_backoff_ticks: u32,
    random_factor_permille: u16,
    max_retries: usize,
    flow: Flow<In, Out, Mat>,
    decide_retry: R,
  ) -> Flow<In, Out, Mat>
  where
    In: Clone + Send + Sync + 'static,
    Out: Send + Sync + 'static,
    Mat: Default + Send + 'static,
    R: Fn(&In, &Out) -> Option<In> + Send + 'static, {
    let (inner_logics, _inner_mat) = flow.into_logics();
    let definition = retry_flow_definition::<In, Out, R>(
      inner_logics,
      decide_retry,
      max_retries,
      min_backoff_ticks,
      max_backoff_ticks,
      random_factor_permille,
    );
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(definition));
    Flow::from_graph(graph, Mat::default())
  }

  /// Creates a context-preserving retry flow with exponential backoff.
  ///
  /// This is the context-aware counterpart of [`with_backoff`](Self::with_backoff).
  /// The inner `FlowWithContext` is unwrapped to its tuple flow
  /// `Flow<(Ctx, In), (Ctx, Out), Mat>`, retried with the same backoff logic,
  /// and re-wrapped into a `FlowWithContext`.
  ///
  /// # Parameters
  ///
  /// * `min_backoff_ticks` – minimum delay (in ticks) before the first retry.
  /// * `max_backoff_ticks` – upper bound for the backoff delay.
  /// * `random_factor_permille` – jitter factor in permille (0–1000). `0` disables jitter.
  /// * `max_retries` – maximum number of retry attempts per element.
  /// * `flow` – the inner context-preserving flow whose output is checked for retries.
  /// * `decide_retry` – called with `(&(ctx, input), &(ctx, output))`. Returns `Some((ctx,
  ///   retry_element))` to trigger a retry, or `None` to accept.
  ///
  /// # Type constraints
  ///
  /// * `Ctx: Clone` – context values must be cloneable for retry re-emission.
  /// * `In: Clone` – elements must be cloneable so they can be re-sent on retry.
  /// * `Mat: Default` – the materialized value of the returned flow uses the default.
  #[must_use]
  pub fn with_backoff_and_context<Ctx, In, Out, Mat, R>(
    min_backoff_ticks: u32,
    max_backoff_ticks: u32,
    random_factor_permille: u16,
    max_retries: usize,
    flow: FlowWithContext<Ctx, In, Out, Mat>,
    decide_retry: R,
  ) -> FlowWithContext<Ctx, In, Out, Mat>
  where
    Ctx: Clone + Send + Sync + 'static,
    In: Clone + Send + Sync + 'static,
    Out: Send + Sync + 'static,
    Mat: Default + Send + 'static,
    R: Fn(&(Ctx, In), &(Ctx, Out)) -> Option<(Ctx, In)> + Send + 'static, {
    let inner_flow = flow.into_flow();
    let retry_flow = Self::with_backoff(
      min_backoff_ticks,
      max_backoff_ticks,
      random_factor_permille,
      max_retries,
      inner_flow,
      decide_retry,
    );
    FlowWithContext::from_flow(retry_flow)
  }
}
