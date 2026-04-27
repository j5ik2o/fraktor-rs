//! Outbound drain loop: pulls envelopes from an `Association` and forwards
//! them through the transport.

use core::{future::Future, time::Duration};

use fraktor_remote_core_rs::core::{
  address::Address,
  extension::EventPublisher,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
};
use fraktor_utils_core_rs::core::sync::SharedLock;
use tokio::time::{Instant, sleep, timeout};

use crate::std::association_runtime::{
  apply_effects_in_place, association_shared::AssociationShared, reconnect_backoff_policy::ReconnectBackoffPolicy,
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
  event_publisher: EventPublisher,
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
        // `RemoteTransport::send` は envelope を所有権で受け取り、失敗時にも返してくれない。
        // `next_outbound` で send_queue から既に取り出してしまっているため、送信失敗を
        // 復旧パスに乗せる前にコピーを保持しておき、reconnect 完了後に
        // `Association::enqueue` で再投入する。これにより一過性の transport 失敗で
        // メッセージが silent-drop されることを防ぐ。
        let envelope_for_retry = envelope.clone();
        let send_result = transport.with_lock(|transport| transport.send(envelope));
        match send_result {
          | Ok(()) => {},
          | Err(TransportError::NotStarted) => {
            // shutdown 経路: 取り出し済み envelope を deferred に戻して shutdown 後の再起動で
            // 再送できるようにする。
            shared.with_write(|assoc| {
              let effects = assoc.enqueue(envelope_for_retry);
              apply_effects_in_place(assoc, effects, &event_publisher);
            });
            return Ok(());
          },
          | Err(err) => {
            gate_for_reconnect(&shared, &policy, elapsed_ms(started_at), &event_publisher);
            let ctx =
              RecoverContext { shared: &shared, policy: &policy, event_publisher: &event_publisher, started_at };
            recover_with_restart_budget(ctx, &mut reconnect, remote, &mut restarts, err).await?;
            // recover が成功すると association は Active 経路へ戻るため、保持していた
            // envelope を再投入する。これは Pekko Artery の AckedDeliveryQueue 相当の最低限
            // の振る舞いで、ack ベースの再送は別レイヤの責務として残してある。
            shared.with_write(|assoc| {
              let effects = assoc.enqueue(envelope_for_retry);
              apply_effects_in_place(assoc, effects, &event_publisher);
            });
          },
        }
      },
      | None => sleep(POLL_INTERVAL).await,
    }
  }
}

/// Immutable context bundle passed to [`recover_with_restart_budget`].
///
/// Grouping the unchanging dependencies (shared association handle, backoff
/// policy, event publisher, loop start instant) into a single struct keeps
/// the function under clippy's `too_many_arguments` threshold while still
/// making each member explicit at the call site.
pub(super) struct RecoverContext<'a> {
  pub shared:          &'a AssociationShared,
  pub policy:          &'a ReconnectBackoffPolicy,
  pub event_publisher: &'a EventPublisher,
  pub started_at:      Instant,
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
  ctx: RecoverContext<'_>,
  reconnect: &mut F,
  remote: Address,
  restarts: &mut u32,
  first_error: TransportError,
) -> Result<(), TransportError>
where
  F: FnMut(Address) -> Fut,
  Fut: Future<Output = Result<TransportEndpoint, TransportError>>, {
  let mut last_error = first_error;
  loop {
    if *restarts >= ctx.policy.max_restarts() {
      return Err(last_error);
    }
    *restarts += 1;
    match reconnect_after_backoff(ctx.policy, reconnect, remote.clone()).await {
      | Ok(endpoint) => {
        // Association::recover が生成する PublishLifecycle / StartHandshake などを
        // 単なる debug ログに留めず、apply_effects_in_place 経由で event stream に
        // 流す。これにより observability (Connected lifecycle 等) が reconnect 経路でも
        // 維持される。
        ctx.shared.with_write(|assoc| {
          let effects = assoc.recover(Some(endpoint), elapsed_ms(ctx.started_at));
          apply_effects_in_place(assoc, effects, ctx.event_publisher);
        });
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

fn gate_for_reconnect(
  shared: &AssociationShared,
  policy: &ReconnectBackoffPolicy,
  now_ms: u64,
  event_publisher: &EventPublisher,
) {
  let backoff_ms = duration_millis(policy.backoff());
  // gate() が返す PublishLifecycle(Gated) / DiscardEnvelopes を event stream へ流すため、
  // log_association_effects ではなく apply_effects_in_place を使う。
  shared.with_write(|assoc| {
    let effects = assoc.gate(Some(now_ms.saturating_add(backoff_ms)), now_ms);
    apply_effects_in_place(assoc, effects, event_publisher);
  });
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
  // タイムアウトは TransportError 列挙体に独自バリアントを持たないため、上位に対しては
  // ConnectionClosed として扱う (再接続ループの共通失敗経路に乗せる)。実原因を観測できる
  // よう、マップする前に WARN ログを残す。
  let remote_for_log = remote.clone();
  match timeout(policy.timeout(), reconnect(remote)).await {
    | Ok(result) => result,
    | Err(_elapsed) => {
      tracing::warn!(remote = %remote_for_log, timeout_ms = duration_millis(policy.timeout()), "reconnect attempt timed out");
      Err(TransportError::ConnectionClosed)
    },
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
