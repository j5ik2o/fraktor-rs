//! Failure detector abstractions and implementations.

/// Deadline-based failure detector.
pub mod deadline_failure_detector;
mod default_failure_detector_registry;
mod failure_detector;
mod failure_detector_registry;
mod failure_detector_with_address;
/// Phi accrual failure detector.
pub mod phi_failure_detector;

pub use default_failure_detector_registry::DefaultFailureDetectorRegistry;
pub use failure_detector::FailureDetector;
pub use failure_detector_registry::FailureDetectorRegistry;
pub use failure_detector_with_address::FailureDetectorWithAddress;
