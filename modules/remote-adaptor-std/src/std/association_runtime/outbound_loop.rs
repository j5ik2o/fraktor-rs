//! Outbound drain loop: pulls envelopes from an `Association` and forwards
//! them through the transport.

use core::{future::Future, time::Duration};

use fraktor_remote_core_rs::core::{
  address::Address,
  association::AssociationEffect,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
};
use fraktor_utils_core_rs::core::sync::SharedLock;
use tokio::time::{Instant, sleep, timeout};

use crate::std::association_runtime::{
  association_shared::AssociationShared, reconnect_backoff_policy::ReconnectBackoffPolicy,
};

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

/// Drains outbound envelopes and reconnects after transient transport failures.
///
/// `TransportError::NotStarted` is treated as shutdown. `SendFailed` and
/// `ConnectionClosed` gate the association, wait for the configured backoff,
/// run the supplied reconnect operation, and recover the association into a new
/// handshake when reconnect succeeds.
///
/// # Errors
///
/// Returns the observed transport error when the restart budget is exhausted or
/// when a reconnect attempt times out after consuming the available budget.
pub async fn run_outbound_loop_with_reconnect<T, F, Fut>(
  shared: AssociationShared,
  transport: SharedLock<T>,
  policy: ReconnectBackoffPolicy,
  mut reconnect: F,
) -> Result<(), TransportError>
where
  T: RemoteTransport + Send + 'static,
  F: FnMut(Address) -> Fut + Send + 'static,
  Fut: Future<Output = Result<TransportEndpoint, TransportError>> + Send, {
  let started_at = Instant::now();
  let mut restarts = 0;
  loop {
    let (remote, next_envelope) = shared.with_write(|assoc| (assoc.remote().clone(), assoc.next_outbound()));
    match next_envelope {
      | Some(envelope) => {
        let send_result = transport.with_lock(|transport| transport.send(envelope));
        match send_result {
          | Ok(()) => {},
          | Err(TransportError::NotStarted) => return Ok(()),
          | Err(err) => {
            gate_for_reconnect(&shared, &policy, elapsed_ms(started_at));
            recover_with_restart_budget(&shared, &policy, &mut reconnect, remote, started_at, &mut restarts, err)
              .await?;
          },
        }
      },
      | None => sleep(POLL_INTERVAL).await,
    }
  }
}

/// Drives the reconnect retry loop bounded by `policy.max_restarts()`.
///
/// On a successful reconnect the function resets `*restarts` to `0` so a
/// future independent failure cycle starts from a fresh budget rather than
/// inheriting the consumed credits from prior recoveries.
///
/// Visibility is `pub(super)` to allow direct unit testing of the budget
/// reset behaviour from the sibling test module without needing to drive a
/// full transport / handshake fixture.
pub(super) async fn recover_with_restart_budget<F, Fut>(
  shared: &AssociationShared,
  policy: &ReconnectBackoffPolicy,
  reconnect: &mut F,
  remote: Address,
  started_at: Instant,
  restarts: &mut u32,
  first_error: TransportError,
) -> Result<(), TransportError>
where
  F: FnMut(Address) -> Fut,
  Fut: Future<Output = Result<TransportEndpoint, TransportError>>, {
  let mut last_error = first_error;
  loop {
    if *restarts >= policy.max_restarts() {
      return Err(last_error);
    }
    *restarts += 1;
    match reconnect_after_backoff(policy, reconnect, remote.clone()).await {
      | Ok(endpoint) => {
        let effects = shared.with_write(|assoc| assoc.recover(Some(endpoint), elapsed_ms(started_at)));
        log_association_effects(effects);
        // Reset the restart counter on successful recovery so a future
        // transient failure long after this point still has the full
        // restart budget rather than the residue from prior failures.
        *restarts = 0;
        return Ok(());
      },
      | Err(err) => {
        last_error = err;
      },
    }
  }
}

fn gate_for_reconnect(shared: &AssociationShared, policy: &ReconnectBackoffPolicy, now_ms: u64) {
  let backoff_ms = duration_millis(policy.backoff());
  let effects = shared.with_write(|assoc| assoc.gate(Some(now_ms.saturating_add(backoff_ms)), now_ms));
  log_association_effects(effects);
}

async fn reconnect_after_backoff<F, Fut>(
  policy: &ReconnectBackoffPolicy,
  reconnect: &mut F,
  remote: Address,
) -> Result<TransportEndpoint, TransportError>
where
  F: FnMut(Address) -> Fut,
  Fut: Future<Output = Result<TransportEndpoint, TransportError>>, {
  sleep(policy.backoff()).await;
  match timeout(policy.timeout(), reconnect(remote)).await {
    | Ok(result) => result,
    | Err(_elapsed) => Err(TransportError::ConnectionClosed),
  }
}

fn elapsed_ms(started_at: Instant) -> u64 {
  duration_millis(started_at.elapsed())
}

fn duration_millis(duration: Duration) -> u64 {
  let millis = duration.as_millis();
  match u64::try_from(millis) {
    | Ok(value) => value,
    | Err(_overflow) => u64::MAX,
  }
}

fn log_association_effects(effects: Vec<AssociationEffect>) {
  for effect in effects {
    tracing::debug!(?effect, "outbound reconnect observed association effect");
  }
}
