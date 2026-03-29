//! GraphStage primitives and stage helpers.
//!
//! ```compile_fail
//! use fraktor_stream_rs::core::stage::{flow::Flow, sink::Sink, source::Source};
//! ```

use alloc::{boxed::Box, vec::Vec};

// Bridge imports from core level for children
use super::{FlowDefinition, FlowLogic, materialization::MatCombineRule};

/// Async callback queue for stage logic.
mod async_callback;
/// Stage execution context.
mod stage_context;
/// Built-in stage kinds.
mod stage_kind;
/// Stream stage trait.
mod stream_stage;
/// Timer helper for stage logic.
mod timer_graph_stage_logic;

pub use async_callback::AsyncCallback;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_stage::StreamStage;
pub use timer_graph_stage_logic::TimerGraphStageLogic;

pub(in crate::core) type Flow<In, Out, Mat> = super::dsl_contract::Flow<In, Out, Mat>;
pub(in crate::core) type Sink<In, Mat> = super::dsl_contract::Sink<In, Mat>;
pub(in crate::core) type Source<Out, Mat> = super::dsl_contract::Source<Out, Mat>;
pub(in crate::core) type TailSource<Out> = super::dsl_contract::TailSource<Out>;

pub(in crate::core) fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  crate::core::dsl::combine_mat::<Left, Right, C>(left, right)
}

pub(in crate::core) fn retry_flow_definition<In, Out, R>(
  inner_logics: Vec<Box<dyn FlowLogic>>,
  decide_retry: R,
  max_retries: usize,
  min_backoff_ticks: u32,
  max_backoff_ticks: u32,
  random_factor_permille: u16,
) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static,
  Out: Send + Sync + 'static,
  R: Fn(&In, &Out) -> Option<In> + Send + 'static, {
  crate::core::dsl::retry_flow_definition::<In, Out, R>(
    inner_logics,
    decide_retry,
    max_retries,
    min_backoff_ticks,
    max_backoff_ticks,
    random_factor_permille,
  )
}

/// Extracts the last context and collects values from a context-value pair sequence.
///
/// Used by `FlowWithContext` and `SourceWithContext` for `grouped` / `sliding`.
pub(crate) fn extract_last_ctx_and_values<Ctx, V>(pairs: Vec<(Ctx, V)>) -> Option<(Ctx, Vec<V>)> {
  let mut last_ctx = None;
  let values: Vec<V> = pairs
    .into_iter()
    .map(|(ctx, v)| {
      last_ctx = Some(ctx);
      v
    })
    .collect();
  last_ctx.map(|ctx| (ctx, values))
}
