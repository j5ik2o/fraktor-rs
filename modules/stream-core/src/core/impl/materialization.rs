//! Internal materialization implementation namespace.

use fraktor_actor_core_rs::core::kernel::actor::scheduler::SchedulerHandle;
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

mod stream;
mod stream_island_actor;
mod stream_island_command;
mod stream_island_drive_gate;
mod stream_shared;
mod stream_state;

pub(in crate::core) type StreamIslandTickHandleSlot = ArcShared<SpinSyncMutex<Option<SchedulerHandle>>>;

pub(crate) use stream::Stream;
pub(crate) use stream_island_actor::StreamIslandActor;
pub(crate) use stream_island_command::StreamIslandCommand;
pub(crate) use stream_island_drive_gate::StreamIslandDriveGate;
pub(crate) use stream_shared::StreamShared;
pub(crate) use stream_state::StreamState;
