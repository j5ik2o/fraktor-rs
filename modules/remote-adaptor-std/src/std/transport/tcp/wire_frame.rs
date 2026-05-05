//! TCP transport alias for the decoded core wire frame.

use fraktor_remote_core_rs::core::wire::WireFrame as CoreWireFrame;

/// Decoded wire frame sent and received by the TCP codec.
pub type WireFrame = CoreWireFrame;
