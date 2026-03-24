//! Materialization pipeline.

// Bridge imports from core level for children
use super::{
  StreamBufferConfig, StreamError, StreamPlan,
  lifecycle::{self, SharedKillSwitch, StreamHandleId, StreamHandleImpl, UniqueKillSwitch},
};

mod actor_materializer;
mod actor_materializer_config;
mod materialized;
mod materializer;
mod materializer_lifecycle_state;
mod materializer_snapshot;
mod runnable_graph;

pub use actor_materializer::ActorMaterializer;
pub use actor_materializer_config::ActorMaterializerConfig;
pub use materialized::Materialized;
pub use materializer::Materializer;
pub use materializer_lifecycle_state::MaterializerLifecycleState;
pub use materializer_snapshot::MaterializerSnapshot;
pub use runnable_graph::RunnableGraph;
