use super::ClusterMessagePayloadKind;

#[test]
fn payload_kind_tags_are_stable() {
  assert_eq!(ClusterMessagePayloadKind::Gossip.tag(), 1);
  assert_eq!(ClusterMessagePayloadKind::PubSub.tag(), 2);
}

#[test]
fn payload_kind_decodes_known_tags() {
  assert_eq!(ClusterMessagePayloadKind::from_tag(1), Some(ClusterMessagePayloadKind::Gossip));
  assert_eq!(ClusterMessagePayloadKind::from_tag(2), Some(ClusterMessagePayloadKind::PubSub));
}

#[test]
fn payload_kind_does_not_round_unknown_tag_to_known_kind() {
  assert_eq!(ClusterMessagePayloadKind::from_tag(0), None);
  assert_eq!(ClusterMessagePayloadKind::from_tag(99), None);
}
