//! GraphStage primitives and stage helpers.
//!
//! ```compile_fail
//! use fraktor_stream_core_rs::stage::{flow::Flow, sink::Sink, source::Source};
//! ```

use alloc::vec::Vec;

// Bridge imports from core level for children
use super::StreamError;

/// Async callback queue for stage logic.
mod async_callback;
/// Non-failure cancellation carrier.
mod cancellation_cause;
/// Non-failure cancellation reason.
mod cancellation_kind;
/// Eagerly propagates upstream termination events.
mod eager_terminate_input;
/// Eagerly propagates downstream cancellation events.
mod eager_terminate_output;
/// Graph stage definition.
mod graph_stage;
/// Graph stage processing logic.
mod graph_stage_logic;
/// Swallows upstream completion while propagating failures.
mod ignore_terminate_input;
/// Swallows downstream cancellation events.
mod ignore_terminate_output;
/// Input-side stage handler trait.
mod in_handler;
/// Kill-switch-aware stage logic mixin.
mod killable_graph_stage_logic;
/// Output-side stage handler trait.
mod out_handler;
/// Actor-like handle bound to graph stage lifecycle.
mod stage_actor;
/// Message envelope delivered to a stage actor.
mod stage_actor_envelope;
/// Stage actor receive callback trait.
mod stage_actor_receive;
/// Stage execution context.
mod stage_context;
/// Built-in stage kinds.
mod stage_kind;
/// Stage-level logging facade.
mod stage_logging;
/// Stream stage trait.
mod stream_stage;
/// Dynamic sub-sink inlet.
mod sub_sink_inlet;
/// Dynamic sub-sink inlet handler trait.
mod sub_sink_inlet_handler;
/// Dynamic sub-source outlet.
mod sub_source_outlet;
/// Dynamic sub-source outlet handler trait.
mod sub_source_outlet_handler;
/// Timer helper for stage logic.
mod timer_graph_stage_logic;
/// Absorbs every upstream event including failures.
mod totally_ignorant_input;

pub use async_callback::AsyncCallback;
pub use cancellation_cause::CancellationCause;
pub use cancellation_kind::CancellationKind;
pub use eager_terminate_input::EagerTerminateInput;
pub use eager_terminate_output::EagerTerminateOutput;
pub use graph_stage::GraphStage;
pub use graph_stage_logic::GraphStageLogic;
pub use ignore_terminate_input::IgnoreTerminateInput;
pub use ignore_terminate_output::IgnoreTerminateOutput;
pub use in_handler::InHandler;
pub use killable_graph_stage_logic::KillableGraphStageLogic;
pub use out_handler::OutHandler;
pub use stage_actor::StageActor;
pub use stage_actor_envelope::StageActorEnvelope;
pub use stage_actor_receive::StageActorReceive;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stage_logging::StageLogging;
pub use stream_stage::StreamStage;
pub use sub_sink_inlet::SubSinkInlet;
pub use sub_sink_inlet_handler::SubSinkInletHandler;
pub use sub_source_outlet::SubSourceOutlet;
pub use sub_source_outlet_handler::SubSourceOutletHandler;
pub use timer_graph_stage_logic::TimerGraphStageLogic;
pub use totally_ignorant_input::TotallyIgnorantInput;

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
