//! Buffer, backpressure, and demand management.

// Bridge imports from core level for children
use super::StreamError;

mod completion_strategy;
mod demand;
mod demand_tracker;
mod overflow_strategy;
mod stream_buffer;
mod stream_buffer_config;

pub use completion_strategy::CompletionStrategy;
pub use demand::Demand;
pub use demand_tracker::DemandTracker;
pub use overflow_strategy::OverflowStrategy;
pub use stream_buffer::StreamBuffer;
pub use stream_buffer_config::StreamBufferConfig;
