//! Stream topology shapes and connection points.

mod bidi_shape;
mod closed_shape;
mod flow_shape;
mod inlet;
mod outlet;
mod port_id;
#[allow(clippy::module_inception)]
mod shape;
mod sink_shape;
mod source_shape;
mod stream_shape;
mod uniform_fan_in_shape;

pub use bidi_shape::BidiShape;
pub use closed_shape::ClosedShape;
pub use flow_shape::FlowShape;
pub use inlet::Inlet;
pub use outlet::Outlet;
pub use port_id::PortId;
pub use shape::Shape;
pub use sink_shape::SinkShape;
pub use source_shape::SourceShape;
pub use stream_shape::StreamShape;
pub use uniform_fan_in_shape::UniformFanInShape;
