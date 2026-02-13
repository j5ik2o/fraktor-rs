//! Materialization pipeline.

// Bridge imports from core level for children
use super::{
  StreamBufferConfig, StreamError, StreamPlan,
  lifecycle::{self, SharedKillSwitch, StreamHandleGeneric, StreamHandleId, UniqueKillSwitch},
};

mod actor_materializer;
mod actor_materializer_config;
mod materialized;
mod materializer;
mod runnable_graph;

pub use actor_materializer::ActorMaterializerGeneric;
pub use actor_materializer_config::ActorMaterializerConfig;
pub use materialized::Materialized;
pub use materializer::Materializer;
pub use runnable_graph::RunnableGraph;
