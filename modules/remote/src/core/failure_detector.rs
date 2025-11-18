//! Failure detector primitives.

mod failure_detector_event;
mod phi_failure_detector;
mod phi_failure_detector_config;

pub use failure_detector_event::FailureDetectorEvent;
pub use phi_failure_detector::PhiFailureDetector;
pub use phi_failure_detector_config::PhiFailureDetectorConfig;
