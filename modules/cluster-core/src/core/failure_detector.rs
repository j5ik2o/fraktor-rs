//! Failure detector traits and registry used by the cluster layer.
//!
//! Moved here from the legacy `fraktor-remote-rs::core::failure_detector`
//! during the `remote-redesign` change. Cluster-level failure tracking is a
//! cluster concern, so the redesign keeps the abstractions next to their
//! only consumer (`fraktor-cluster-core-rs`).
//!
//! For the actual Phi Accrual algorithm see the `PhiAccrualFailureDetector`
//! type in `fraktor-remote-core-rs`. The cluster layer uses these trait
//! abstractions so it can plug arbitrary detector implementations (real Phi
//! Accrual or test stubs) per-resource.

mod default_failure_detector_registry;
mod failure_detector;
mod failure_detector_registry;
/// The legacy `PhiFailureDetector` is exposed as a public sub-module.
/// Re-exporting `PhiFailureDetector` / `PhiFailureDetectorConfig` from this
/// grandparent module would violate the `no-parent-reexport` dylint rule
/// (because `phi_failure_detector` itself has a child `config` module and
/// is therefore not a leaf), so consumers import via the full path
/// `failure_detector::phi_failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig}`.
pub mod phi_failure_detector;

pub use default_failure_detector_registry::DefaultFailureDetectorRegistry;
pub use failure_detector::FailureDetector;
pub use failure_detector_registry::FailureDetectorRegistry;
