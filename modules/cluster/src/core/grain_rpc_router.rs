//! Concurrency- and schema-aware RPC router.

use alloc::{
  collections::{BTreeMap, VecDeque},
  string::ToString,
  vec::Vec,
};

use crate::core::{
  dispatch_drop_policy::DispatchDropPolicy, grain_key::GrainKey, rpc_dispatch::RpcDispatch, rpc_error::RpcError,
  rpc_event::RpcEvent, schema_negotiator::SchemaNegotiator, serialized_message::SerializedMessage,
};

#[cfg(test)]
mod tests;

struct KeyState {
  in_flight: u32,
  queue:     VecDeque<Queued>,
}

struct Queued {
  message:  SerializedMessage,
  deadline: u64,
}

/// Router that enforces schema compatibility, serialization validation, and per-key concurrency
/// limits.
pub struct GrainRpcRouter {
  concurrency_limit: u32,
  queue_depth:       usize,
  drop_policy:       DispatchDropPolicy,
  negotiator:        SchemaNegotiator,
  negotiated:        Option<u32>,
  states:            BTreeMap<GrainKey, KeyState>,
  events:            Vec<RpcEvent>,
}

impl GrainRpcRouter {
  /// Creates a new router.
  #[must_use]
  pub const fn new(
    concurrency_limit: u32,
    queue_depth: usize,
    drop_policy: DispatchDropPolicy,
    supported_versions: Vec<u32>,
  ) -> Self {
    Self {
      concurrency_limit,
      queue_depth,
      drop_policy,
      negotiator: SchemaNegotiator::new(supported_versions),
      negotiated: None,
      states: BTreeMap::new(),
      events: Vec::new(),
    }
  }

  /// Performs schema negotiation with peer and stores the agreed version.
  pub fn negotiate(&mut self, peer: &[u32]) -> Option<u32> {
    self.negotiated = self.negotiator.negotiate(peer);
    self.negotiated
  }

  /// Dispatches a request, applying concurrency, queue, and schema rules.
  ///
  /// # Errors
  ///
  /// Returns `RpcError::EmptyPayload` if the message payload is empty.
  /// Returns `RpcError::SchemaIncompatible` if schema negotiation has not been performed or the
  /// message version is incompatible. Returns `RpcError::Dropped` if the queue is full and the
  /// drop policy results in rejection.
  pub fn dispatch(
    &mut self,
    key: GrainKey,
    message: SerializedMessage,
    deadline: u64,
  ) -> Result<RpcDispatch, RpcError> {
    let mut local_events = Vec::new();

    if message.is_empty() {
      let reason = "empty payload".to_string();
      local_events.push(RpcEvent::SerializationFailed { key: key.clone(), reason: reason.clone() });
      self.events.extend(local_events);
      return Err(RpcError::SerializationFailed { reason });
    }

    if let Some(version) = self.negotiated {
      if version != message.schema_version {
        local_events
          .push(RpcEvent::SchemaMismatch { key: key.clone(), message_version: message.schema_version });
        self.events.extend(local_events);
        return Err(RpcError::SchemaMismatch {
          negotiated:      Some(version),
          message_version: message.schema_version,
        });
      }
    } else {
      self.events.extend(local_events);
      return Err(RpcError::SchemaMismatch { negotiated: None, message_version: message.schema_version });
    }

    let concurrency_limit = self.concurrency_limit;
    let queue_depth = self.queue_depth;
    let drop_policy = self.drop_policy;

    let action = {
      let state = self.state_mut(&key);

      if state.in_flight < concurrency_limit {
        state.in_flight += 1;
        local_events.push(RpcEvent::Dispatched { key: key.clone(), deadline });
        RpcDispatch::Immediate { key, message, deadline }
      } else if state.queue.len() < queue_depth {
        state.queue.push_back(Queued { message, deadline });
        let len = state.queue.len();
        local_events.push(RpcEvent::Queued { key, queue_len: len });
        RpcDispatch::Queued { queue_len: len }
      } else {
        match drop_policy {
          | DispatchDropPolicy::RejectNew => {
            local_events.push(RpcEvent::Rejected { key, reason: "queue_full".to_string() });
            RpcDispatch::Dropped { reason: "queue_full".to_string() }
          },
          | DispatchDropPolicy::DropOldest => {
            if state.queue.pop_front().is_some() {
              state.queue.push_back(Queued { message, deadline });
              let len = state.queue.len();
              local_events.push(RpcEvent::DroppedOldest { key: key.clone(), reason: "queue_overflow".to_string() });
              RpcDispatch::Queued { queue_len: len }
            } else {
              local_events.push(RpcEvent::Rejected { key, reason: "queue_full".to_string() });
              RpcDispatch::Dropped { reason: "queue_full".to_string() }
            }
          },
        }
      }
    };

    self.events.extend(local_events);
    Ok(action)
  }

  /// Marks a request complete; promotes next queued item if present.
  pub fn complete(&mut self, key: &GrainKey, now: u64) -> Option<RpcDispatch> {
    let mut local_events = Vec::new();
    let action = {
      let state = self.state_mut(key);
      if state.in_flight > 0 {
        state.in_flight -= 1;
      }

      if let Some(next) = state.queue.pop_front() {
        state.in_flight += 1;
        if now >= next.deadline {
          local_events.push(RpcEvent::TimedOut { key: key.clone() });
          Some(RpcDispatch::Dropped { reason: "timeout".to_string() })
        } else {
          local_events.push(RpcEvent::Promoted { key: key.clone() });
          Some(RpcDispatch::Immediate { key: key.clone(), message: next.message, deadline: next.deadline })
        }
      } else {
        None
      }
    };
    self.events.extend(local_events);
    action
  }

  /// Drains emitted events.
  pub fn drain_events(&mut self) -> Vec<RpcEvent> {
    core::mem::take(&mut self.events)
  }

  fn state_mut(&mut self, key: &GrainKey) -> &mut KeyState {
    self.states.entry(key.clone()).or_insert_with(|| KeyState { in_flight: 0, queue: VecDeque::new() })
  }
}
