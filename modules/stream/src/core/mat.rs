//! Materialization pipeline.

// Bridge imports from core level for children
use super::{
  Completion, StreamError, StreamNotUsed, StreamPlan,
  buffer::StreamBufferConfig,
  lifecycle::{self, SharedKillSwitch, StreamHandleId, StreamHandleImpl, UniqueKillSwitch},
};

mod actor_materializer;
mod actor_materializer_config;
mod keep_both;
mod keep_left;
mod keep_none;
mod keep_right;
mod mat_combine;
mod mat_combine_rule;
mod materialized;
mod materializer;
mod materializer_lifecycle_state;
mod materializer_snapshot;
mod runnable_graph;
mod stream_completion;

pub use actor_materializer::ActorMaterializer;
pub use actor_materializer_config::ActorMaterializerConfig;
pub use keep_both::KeepBoth;
pub use keep_left::KeepLeft;
pub use keep_none::KeepNone;
pub use keep_right::KeepRight;
pub use mat_combine::MatCombine;
pub use mat_combine_rule::MatCombineRule;
pub use materialized::Materialized;
pub use materializer::Materializer;
pub use materializer_lifecycle_state::MaterializerLifecycleState;
pub use materializer_snapshot::MaterializerSnapshot;
pub use runnable_graph::RunnableGraph;
pub use stream_completion::StreamCompletion;
