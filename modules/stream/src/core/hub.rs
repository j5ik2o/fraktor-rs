//! Dynamic fan-in/fan-out connectors.

// Bridge imports for children
use super::{
  DynValue, SinkDecision, SinkLogic, SourceLogic, StreamError, StreamNotUsed,
  buffer::DemandTracker,
  downcast_value,
  dsl::{Sink, Source},
  stage::StageKind,
};

mod broadcast_hub;
mod draining_control;
mod merge_hub;
mod partition_hub;

pub(in crate::core) use broadcast_hub::BroadcastHub;
pub(in crate::core) use draining_control::DrainingControl;
pub(in crate::core) use merge_hub::MergeHub;
pub(in crate::core) use partition_hub::PartitionHub;
