use tokio::task::JoinHandle;

/// Handle controlling bridge background tasks.
pub struct EndpointTransportBridgeHandle {
  pub(super) send_task: JoinHandle<()>,
}

impl EndpointTransportBridgeHandle {
  /// Aborts the background outbound loop.
  pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
    self.send_task.abort();
    self.send_task.await
  }
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  };

  use tokio::{
    task::yield_now,
    time::{Duration, sleep},
  };

  use super::EndpointTransportBridgeHandle;

  struct TaskDropProbe {
    dropped: Arc<AtomicBool>,
  }

  impl Drop for TaskDropProbe {
    fn drop(&mut self) {
      self.dropped.store(true, Ordering::SeqCst);
    }
  }

  #[tokio::test]
  async fn shutdown_waits_for_send_task_completion() {
    let dropped = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let send_task = tokio::spawn({
      let dropped = Arc::clone(&dropped);
      let started = Arc::clone(&started);
      async move {
        started.store(true, Ordering::SeqCst);
        let _probe = TaskDropProbe { dropped };
        loop {
          sleep(Duration::from_millis(50)).await;
        }
      }
    });

    while !started.load(Ordering::Acquire) {
      yield_now().await;
    }

    let handle = EndpointTransportBridgeHandle { send_task };
    let _ = handle.shutdown().await.expect_err("send task should be cancelled");

    assert!(dropped.load(Ordering::SeqCst));
  }
}
