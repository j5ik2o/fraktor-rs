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
///   defers them again or surfaces a recursive `DiscardEnvelopes` effect, which is logged below.
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
  for effect in effects {
    apply_one(assoc, effect, event_publisher);
  }
}

fn apply_one(assoc: &mut Association, effect: AssociationEffect, event_publisher: &EventPublisher) {
  match effect {
    | AssociationEffect::SendEnvelopes { envelopes } => {
      let count = envelopes.len();
      for envelope in envelopes {
        let recursive = assoc.enqueue(envelope);
        for inner in recursive {
          apply_recursive_effect(inner, event_publisher);
        }
      }
      tracing::debug!(count, "association re-enqueued deferred envelopes after handshake");
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
      // The transport-driven start path is the caller's responsibility; this
      // helper only logs that the effect was observed.
      tracing::debug!(?endpoint, "association requested handshake start");
    },
  }
}

fn apply_recursive_effect(effect: AssociationEffect, event_publisher: &EventPublisher) {
  match effect {
    | AssociationEffect::DiscardEnvelopes { reason, envelopes } => {
      tracing::warn!(
        discarded = envelopes.len(),
        reason = %reason.message(),
        "association discarded re-enqueued envelopes",
      );
    },
    | AssociationEffect::SendEnvelopes { envelopes } => {
      tracing::debug!(count = envelopes.len(), "association produced nested SendEnvelopes during re-enqueue");
    },
    | AssociationEffect::PublishLifecycle(event) => {
      // tracing は運用観測、event stream は下流購読者向けなので両方へ出力する。
      tracing::info!(?event, "remoting lifecycle event during re-enqueue");
      event_publisher.publish_lifecycle(event);
    },
    | AssociationEffect::StartHandshake { endpoint } => {
      tracing::debug!(?endpoint, "association requested handshake start during re-enqueue");
    },
  }
}
