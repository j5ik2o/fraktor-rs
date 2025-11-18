#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), no_std)]

//! Remoting facilities for the fraktor actor runtime.

extern crate alloc;

mod core;
mod std;

pub use core::{
  backpressure_listener::RemotingBackpressureListener,
  endpoint_reader::EndpointReader,
  endpoint_writer::{EndpointWriter, OutboundEnvelope, RemotingEnvelope},
  failure_detector::{
    failure_detector_event::FailureDetectorEvent, phi_failure_detector::PhiFailureDetector,
    phi_failure_detector_config::PhiFailureDetectorConfig,
  },
  flight_recorder::{
    correlation_trace::{CorrelationTrace, CorrelationTraceHop},
    remoting_flight_recorder::RemotingFlightRecorder,
    remoting_metric::RemotingMetric,
  },
  inbound_envelope::InboundEnvelope,
  remoting_connection_snapshot::RemotingConnectionSnapshot,
  remoting_control::RemotingControl,
  remoting_control_handle::RemotingControlHandle,
  remoting_error::RemotingError,
  remoting_extension::RemotingExtension,
  remoting_extension_config::RemotingExtensionConfig,
  remoting_extension_id::RemotingExtensionId,
};
