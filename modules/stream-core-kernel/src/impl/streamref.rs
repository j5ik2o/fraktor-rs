//! Internal stream reference implementation namespace.

mod stream_ref_endpoint_cleanup;
mod stream_ref_endpoint_slot;
mod stream_ref_endpoint_state;
mod stream_ref_handoff;
mod stream_ref_protocol;
mod stream_ref_sink_logic;
mod stream_ref_source_logic;

pub(crate) use stream_ref_endpoint_slot::StreamRefEndpointSlot;
pub(crate) use stream_ref_handoff::StreamRefHandoff;
pub(crate) use stream_ref_sink_logic::StreamRefSinkLogic;
pub(crate) use stream_ref_source_logic::StreamRefSourceLogic;
