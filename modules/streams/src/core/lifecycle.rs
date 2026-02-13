//! Stream lifecycle and execution management.

// Bridge imports from core level for children
use super::{StreamBufferConfig, StreamError, StreamPlan, graph::GraphInterpreter};

mod drive_outcome;
mod shared_kill_switch;
mod stream;
mod stream_drive_actor;
mod stream_drive_command;
mod stream_handle;
mod stream_handle_generic;
mod stream_handle_id;
mod stream_shared;
mod stream_state;
mod unique_kill_switch;

pub use drive_outcome::DriveOutcome;
pub use shared_kill_switch::SharedKillSwitch;
pub(in crate::core) use stream::Stream;
pub(in crate::core) use stream_drive_actor::StreamDriveActor;
pub(in crate::core) use stream_drive_command::StreamDriveCommand;
pub use stream_handle::StreamHandle;
pub use stream_handle_generic::StreamHandleGeneric;
pub use stream_handle_id::StreamHandleId;
pub(in crate::core) use stream_shared::StreamSharedGeneric;
pub use stream_state::StreamState;
pub(in crate::core) use unique_kill_switch::KillSwitchStateHandle;
pub use unique_kill_switch::UniqueKillSwitch;
