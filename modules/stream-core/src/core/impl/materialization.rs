//! Internal materialization implementation namespace.

mod actor_materializer_runtime;
mod materializer_guard;
mod materializer_session;
mod stream_handle_id;
mod stream_runtime_completion;
mod stream_state;

pub(crate) use actor_materializer_runtime::StreamDriveActor;
pub(crate) use materializer_guard::StreamHandleImpl;
pub(crate) use materializer_session::{Stream, StreamShared};
pub(crate) use stream_handle_id::StreamHandleId;
pub(crate) use stream_runtime_completion::StreamDriveCommand;
pub(crate) use stream_state::StreamState;
