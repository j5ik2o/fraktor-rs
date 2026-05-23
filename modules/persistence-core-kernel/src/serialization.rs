//! Persistence serialization package.

mod message_serializer;
mod registration;
mod snapshot_payload;
mod snapshot_serializer;
mod wire;

pub use message_serializer::MessageSerializer;
pub use registration::{
  MESSAGE_SERIALIZER_ID, PersistenceSerializationContributor, SNAPSHOT_SERIALIZER_ID, register_persistence_serializers,
};
pub use snapshot_payload::SnapshotPayload;
pub use snapshot_serializer::SnapshotSerializer;
