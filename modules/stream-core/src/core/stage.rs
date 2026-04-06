//! GraphStage primitives and stage helpers.
//!
//! ```compile_fail
//! use fraktor_stream_core_rs::core::stage::{flow::Flow, sink::Sink, source::Source};
//! ```

use alloc::vec::Vec;

// Bridge imports from core level for children
use super::StreamError;

/// Async callback queue for stage logic.
mod async_callback;
/// Graph stage definition.
mod graph_stage;
/// Graph stage processing logic.
mod graph_stage_logic;
/// Stage execution context.
mod stage_context;
/// Built-in stage kinds.
mod stage_kind;
/// Stream stage trait.
mod stream_stage;
/// Timer helper for stage logic.
mod timer_graph_stage_logic;

pub use async_callback::AsyncCallback;
pub use graph_stage::GraphStage;
pub use graph_stage_logic::GraphStageLogic;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_stage::StreamStage;
pub use timer_graph_stage_logic::TimerGraphStageLogic;

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
