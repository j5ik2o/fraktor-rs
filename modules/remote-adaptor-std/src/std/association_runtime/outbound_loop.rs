//! Outbound drain loop: pulls envelopes from an `Association` and forwards
//! them through the transport.

use core::time::Duration;

use fraktor_remote_core_rs::core::transport::{RemoteTransport, TransportError};
use fraktor_utils_core_rs::core::sync::SharedLock;
use tokio::time::sleep;

use crate::std::association_runtime::association_shared::AssociationShared;

/// Polling interval used by the outbound drain loop.
const POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Drains [`fraktor_remote_core_rs::core::association::Association::next_outbound`]
/// in a loop, forwarding each envelope through `transport.send`.
///
/// This task is intentionally simple: it polls at `POLL_INTERVAL` whenever
/// the queue is empty. Phase B's minimum-viable implementation does not
/// notify the loop on `enqueue`; instead the polling cadence acts as the
/// flow-control point. The cadence is short enough (1 ms) to keep latency
/// low while still letting the tokio scheduler run other tasks.
///
/// The loop terminates when:
///
/// - the transport reports `TransportError::NotStarted` (the runtime is shutting down), or
/// - `transport.send` reports a connection-level failure (the caller is responsible for re-arming
///   the loop after recovery).
pub async fn run_outbound_loop<T: RemoteTransport + Send + 'static>(
  shared: AssociationShared,
  transport: SharedLock<T>,
) {
  loop {
    let next_envelope = shared.with_write(|assoc| assoc.next_outbound());
    match next_envelope {
      | Some(envelope) => {
        let send_result = transport.with_lock(|transport| transport.send(envelope));
        match send_result {
          | Ok(()) => {},
          | Err(TransportError::NotStarted) => break,
          | Err(_err) => {
            tracing::warn!("outbound loop transport send failed");
            break;
          },
        }
      },
      | None => sleep(POLL_INTERVAL).await,
    }
  }
}
