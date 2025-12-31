//! Messages sent to journal actors.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::persistent_repr::PersistentRepr;

/// Messages sent to the journal actor.
#[derive(Clone, Debug)]
pub enum JournalMessage<TB: RuntimeToolbox + 'static> {
  /// Writes a batch of messages.
  WriteMessages {
    /// Persistence id for the batch.
    persistence_id: String,
    /// Max sequence number within the batch.
    to_sequence_nr: u64,
    /// Events to persist.
    messages:       Vec<PersistentRepr>,
    /// Request sender.
    sender:         ActorRefGeneric<TB>,
    /// Instance id for correlation.
    instance_id:    u32,
  },
  /// Replays messages for a persistence id.
  ReplayMessages {
    /// Persistence id to replay.
    persistence_id:   String,
    /// Starting sequence number.
    from_sequence_nr: u64,
    /// Ending sequence number.
    to_sequence_nr:   u64,
    /// Maximum number of messages to replay.
    max:              u64,
    /// Request sender.
    sender:           ActorRefGeneric<TB>,
  },
  /// Deletes messages up to the given sequence number.
  DeleteMessagesTo {
    /// Persistence id to delete.
    persistence_id: String,
    /// Delete up to this sequence number.
    to_sequence_nr: u64,
    /// Request sender.
    sender:         ActorRefGeneric<TB>,
  },
  /// Requests the highest sequence number.
  GetHighestSequenceNr {
    /// Persistence id to query.
    persistence_id:   String,
    /// Starting sequence number for the query.
    from_sequence_nr: u64,
    /// Request sender.
    sender:           ActorRefGeneric<TB>,
  },
}
