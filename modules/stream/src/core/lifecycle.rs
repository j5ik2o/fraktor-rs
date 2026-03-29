//! Stream lifecycle and execution management.

// Bridge imports from core level for children
use super::{StreamError, StreamPlan, buffer::StreamBufferConfig, r#impl::GraphInterpreter};

mod drive_outcome;
mod kill_switch;
mod kill_switches;
mod shared_kill_switch;
mod stream;
mod stream_drive_actor;
mod stream_drive_command;
mod stream_handle;
mod stream_handle_id;
mod stream_handle_impl;
mod stream_shared;
mod stream_state;
mod unique_kill_switch;

pub use drive_outcome::DriveOutcome;
pub use kill_switch::KillSwitch;
pub use kill_switches::KillSwitches;
pub use shared_kill_switch::SharedKillSwitch;
pub(crate) use stream::Stream;
pub(in crate::core) use stream_drive_actor::StreamDriveActor;
pub(in crate::core) use stream_drive_command::StreamDriveCommand;
pub use stream_handle::StreamHandle;
pub use stream_handle_id::StreamHandleId;
pub use stream_handle_impl::StreamHandleImpl;
pub(crate) use stream_shared::StreamShared;
pub use stream_state::StreamState;
pub use unique_kill_switch::UniqueKillSwitch;
pub(in crate::core) use unique_kill_switch::{KillSwitchState, KillSwitchStateHandle};
