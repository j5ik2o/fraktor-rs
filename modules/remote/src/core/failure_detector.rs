//! Failure detector modules.

mod failure_detector;

/// Phi accrual failure detector for remote nodes.
pub mod phi_failure_detector;

pub use failure_detector::FailureDetector;
