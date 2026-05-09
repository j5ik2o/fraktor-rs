//! Remote log marker helpers.

use alloc::string::{String, ToString};

use fraktor_actor_core_kernel_rs::event::logging::ActorLogMarker;

use crate::core::address::Address;

const FAILURE_DETECTOR_GROWING_MARKER: &str = "pekkoFailureDetectorGrowing";
const QUARANTINE_MARKER: &str = "pekkoQuarantine";
const CONNECT_MARKER: &str = "pekkoConnect";
const DISCONNECTED_MARKER: &str = "pekkoDisconnected";
const REMOTE_ADDRESS_PROPERTY: &str = "pekkoRemoteAddress";
const REMOTE_ADDRESS_UID_PROPERTY: &str = "pekkoRemoteAddressUid";

/// Marker helpers for remote log events.
pub struct RemoteLogMarker;

impl RemoteLogMarker {
  /// Creates a failure-detector-growing marker.
  #[must_use]
  pub fn failure_detector_growing(remote_address: &Address) -> ActorLogMarker {
    ActorLogMarker::new(FAILURE_DETECTOR_GROWING_MARKER)
      .with_property(REMOTE_ADDRESS_PROPERTY, remote_address.to_string())
  }

  /// Creates a quarantine marker.
  #[must_use]
  pub fn quarantine(remote_address: &Address, remote_address_uid: Option<u64>) -> ActorLogMarker {
    Self::marker_with_remote_address_uid(QUARANTINE_MARKER, remote_address, remote_address_uid)
  }

  /// Creates a connect marker.
  #[must_use]
  pub fn connect(remote_address: &Address, remote_address_uid: Option<u64>) -> ActorLogMarker {
    Self::marker_with_remote_address_uid(CONNECT_MARKER, remote_address, remote_address_uid)
  }

  /// Creates a disconnected marker.
  #[must_use]
  pub fn disconnected(remote_address: &Address, remote_address_uid: Option<u64>) -> ActorLogMarker {
    Self::marker_with_remote_address_uid(DISCONNECTED_MARKER, remote_address, remote_address_uid)
  }

  fn marker_with_remote_address_uid(
    name: &'static str,
    remote_address: &Address,
    remote_address_uid: Option<u64>,
  ) -> ActorLogMarker {
    ActorLogMarker::new(name)
      .with_property(REMOTE_ADDRESS_PROPERTY, remote_address.to_string())
      .with_property(REMOTE_ADDRESS_UID_PROPERTY, remote_address_uid_property(remote_address_uid))
  }
}

fn remote_address_uid_property(remote_address_uid: Option<u64>) -> String {
  remote_address_uid.map_or_else(String::new, |value| value.to_string())
}
