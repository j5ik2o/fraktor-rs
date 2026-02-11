//! Stream topology shapes and connection points.

mod bidi_shape;
mod flow_shape;
mod inlet;
mod outlet;
mod port_id;
#[allow(clippy::module_inception)]
mod shape;
mod sink_shape;
mod source_shape;
mod stream_shape;

pub use bidi_shape::BidiShape;
pub use flow_shape::FlowShape;
pub use inlet::Inlet;
pub use outlet::Outlet;
pub use port_id::PortId;
pub use shape::Shape;
pub use sink_shape::SinkShape;
pub use source_shape::SourceShape;
pub use stream_shape::StreamShape;
