//! Internal stream reference implementation namespace.

mod stream_ref_handoff;
mod stream_ref_protocol;
mod stream_ref_sink_logic;
mod stream_ref_source_logic;

pub(in crate::core) use stream_ref_handoff::StreamRefHandoff;
pub(in crate::core) use stream_ref_sink_logic::StreamRefSinkLogic;
pub(in crate::core) use stream_ref_source_logic::StreamRefSourceLogic;
