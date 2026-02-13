use tokio::task::JoinHandle;

/// Handle controlling bridge background tasks.
pub struct EndpointTransportBridgeHandle {
  pub(super) send_task: JoinHandle<()>,
}

impl EndpointTransportBridgeHandle {
  /// Aborts the background outbound loop.
  pub fn shutdown(self) {
    self.send_task.abort();
  }
}
