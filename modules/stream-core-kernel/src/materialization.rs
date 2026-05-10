//! Materialization contracts and lifecycle types.

use super::{SharedKillSwitch, StreamError, StreamPlan, UniqueKillSwitch};

mod actor_materializer;
mod actor_materializer_config;
mod completion;
mod downstream_cancellation_control_plane;
mod downstream_cancellation_route;
mod drive_outcome;
mod keep_both;
mod keep_left;
mod keep_none;
mod keep_right;
mod mat_combine;
mod mat_combine_rule;
mod materialized;
mod materializer;
mod materializer_lifecycle_state;
mod materializer_logging_provider;
mod runnable_graph;
mod stream_done;
mod stream_future;
mod stream_not_used;
mod subscription_timeout_config;
mod subscription_timeout_mode;

pub use actor_materializer::ActorMaterializer;
pub use actor_materializer_config::ActorMaterializerConfig;
pub use completion::Completion;
pub(crate) use downstream_cancellation_control_plane::DownstreamCancellationControlPlaneShared;
pub use drive_outcome::DriveOutcome;
pub use keep_both::KeepBoth;
pub use keep_left::KeepLeft;
pub use keep_none::KeepNone;
pub use keep_right::KeepRight;
pub(crate) use mat_combine::MatCombine;
pub use mat_combine_rule::MatCombineRule;
pub use materialized::Materialized;
pub use materializer::Materializer;
pub use materializer_lifecycle_state::MaterializerLifecycleState;
pub use materializer_logging_provider::MaterializerLoggingProvider;
pub use runnable_graph::RunnableGraph;
pub use stream_done::StreamDone;
pub use stream_future::StreamFuture;
pub use stream_not_used::StreamNotUsed;
pub use subscription_timeout_config::SubscriptionTimeoutConfig;
pub use subscription_timeout_mode::SubscriptionTimeoutMode;

pub(crate) fn empty_downstream_cancellation_control_plane() -> DownstreamCancellationControlPlaneShared {
  downstream_cancellation_control_plane::empty_shared()
}
