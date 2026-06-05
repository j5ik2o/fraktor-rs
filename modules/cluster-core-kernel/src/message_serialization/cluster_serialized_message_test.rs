use alloc::{string::String, vec};

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};

use crate::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};

#[test]
fn serialized_message_preserves_manifest_metadata() {
  let serializer_id = SerializerId::try_from(41).expect("serializer");
  let actor_message =
    SerializedMessage::new(serializer_id, Some(String::from("cluster.payload/gossip")), vec![1, 2, 3]);

  let cluster_message = ClusterSerializedMessage::new(ClusterMessagePayloadKind::Gossip, actor_message);

  assert_eq!(cluster_message.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(cluster_message.serialized_message().serializer_id(), serializer_id);
  assert_eq!(cluster_message.serializer_id(), serializer_id);
  assert_eq!(cluster_message.manifest(), Some("cluster.payload/gossip"));
  assert_eq!(cluster_message.payload_bytes(), &[1, 2, 3]);
}

#[test]
fn serialized_message_preserves_absent_manifest_metadata() {
  let serializer_id = SerializerId::try_from(42).expect("serializer");
  let actor_message = SerializedMessage::new(serializer_id, None, vec![4, 5, 6]);

  let cluster_message = ClusterSerializedMessage::new(ClusterMessagePayloadKind::PubSub, actor_message);

  assert_eq!(cluster_message.payload_kind(), ClusterMessagePayloadKind::PubSub);
  assert_eq!(cluster_message.serialized_message().serializer_id(), serializer_id);
  assert_eq!(cluster_message.serializer_id(), serializer_id);
  assert_eq!(cluster_message.manifest(), None);
  assert_eq!(cluster_message.payload_bytes(), &[4, 5, 6]);
}
