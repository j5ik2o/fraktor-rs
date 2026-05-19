//! Failure detector traits and registry used by the cluster layer.
//!
//! The cluster layer defines the detector trait abstractions; concrete
//! implementations (e.g. `PhiAccrualFailureDetector`) live in
//! `fraktor-remote-core-rs`. Callers plug implementations per-resource
//! via [`FailureDetectorRegistry`].

mod default_failure_detector_registry;
mod failure_detector;
mod failure_detector_registry;

pub use default_failure_detector_registry::DefaultFailureDetectorRegistry;
pub use failure_detector::FailureDetector;
pub use failure_detector_registry::FailureDetectorRegistry;
