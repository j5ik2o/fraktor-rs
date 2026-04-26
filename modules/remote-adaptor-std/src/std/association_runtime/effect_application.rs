//! Helper that materialises [`AssociationEffect`]s emitted by `Association`
//! state transitions.
//!
//! The pure `Association` state machine returns a `Vec<AssociationEffect>` for
//! every transition. The std runtime is responsible for actually performing
//! those side-effects (re-enqueueing flushed envelopes, logging discarded
//! batches, publishing lifecycle events). Discarding the effect vector causes
//! deferred-message loss after a successful handshake — this helper is the
//! single place that the production runtime relies on to avoid that bug.

use fraktor_remote_core_rs::core::{
  association::{Association, AssociationEffect},
  envelope::OutboundEnvelope,
  extension::EventPublisher,
};

/// Applies a batch of [`AssociationEffect`]s in-place against `assoc`.
///
/// `assoc` MUST already be in the state the caller intended (i.e. the state
/// transition method has just returned). This function only realises the
/// side-effects; it never performs further state transitions of its own.
///
/// # Effect handling
///
/// - **`SendEnvelopes`**: each envelope is re-enqueued via `assoc.enqueue` so the outbound loop
///   drains it through `next_outbound`. After `handshake_accepted` the state is `Active`, so the
///   envelopes land in the internal send queue. If the state is anything else, `enqueue` either
///   defers them again or surfaces recursive effects, which are applied below.
/// - **`DiscardEnvelopes`**: the discarded count and reason are logged via `tracing` so the
///   operator can observe the loss.
/// - **`PublishLifecycle`**: published through the actor-system event stream.
/// - **`StartHandshake`**: logged. The transport-driven start path lives on the caller side; this
///   helper is not the place to launch a handshake.
pub fn apply_effects_in_place(
  assoc: &mut Association,
  effects: Vec<AssociationEffect>,
  event_publisher: &EventPublisher,
) {
  // `pending` は LIFO の作業リストなので、再帰を使わずに出力順を保つため reverse してから処理する。
  let mut pending = effects;
  pending.reverse();
  while let Some(effect) = pending.pop() {
    apply_one(assoc, effect, event_publisher, &mut pending);
  }
}

fn apply_one(
  assoc: &mut Association,
  effect: AssociationEffect,
  event_publisher: &EventPublisher,
  pending: &mut Vec<AssociationEffect>,
) {
  match effect {
    | AssociationEffect::SendEnvelopes { envelopes } => {
      apply_send_envelopes(assoc, envelopes, pending);
    },
    | AssociationEffect::DiscardEnvelopes { reason, envelopes } => {
      tracing::warn!(
        discarded = envelopes.len(),
        reason = %reason.message(),
        "association discarded envelopes",
      );
    },
    | AssociationEffect::PublishLifecycle(event) => {
      // tracing は運用観測、event stream は下流購読者向けなので両方へ出力する。
      tracing::info!(?event, "remoting lifecycle event");
      event_publisher.publish_lifecycle(event);
    },
    | AssociationEffect::StartHandshake { endpoint } => {
      // StartHandshake は transport 駆動の開始経路を呼び出し元が担うので、
      // このヘルパーでは観測した効果をログするだけに留める。
      tracing::debug!(?endpoint, "association requested handshake start");
    },
  }
}

fn apply_send_envelopes(
  assoc: &mut Association,
  envelopes: Vec<OutboundEnvelope>,
  pending: &mut Vec<AssociationEffect>,
) {
  let count = envelopes.len();
  let mut recursive = Vec::new();
  for envelope in envelopes {
    recursive.extend(assoc.enqueue(envelope));
  }
  push_effects_in_order(pending, recursive);
  tracing::debug!(count, "association re-enqueued envelopes");
}

fn push_effects_in_order(pending: &mut Vec<AssociationEffect>, effects: Vec<AssociationEffect>) {
  pending.extend(effects.into_iter().rev());
}
