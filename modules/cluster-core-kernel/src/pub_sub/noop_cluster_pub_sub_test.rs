use alloc::{string::String, vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    MediatorCommand, MediatorCommandOutcome, NoopClusterPubSub, PubSubEnvelope, PubSubSubscriber, PubSubTopic,
    cluster_pub_sub::ClusterPubSub,
  },
};

#[test]
fn noop_pubsub_returns_noop_mediator_outcome_after_start() {
  let mut pub_sub = NoopClusterPubSub::new();
  let topic = PubSubTopic::new("news");
  let payload =
    PubSubEnvelope { serializer_id: 1, type_name: String::from("example.Message"), bytes: vec![1] };
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "id").expect("subscriber"));
  let active_owner = UniqueAddress::new(Address::new("cluster", "node-a", 2552), 1);

  pub_sub.start().expect("start");

  assert_eq!(
    pub_sub
      .apply_mediator_command(MediatorCommand::try_publish(topic, payload).expect("publish"), 0, &[active_owner])
      .expect("mediator command"),
    MediatorCommandOutcome::Noop
  );
  assert_eq!(
    pub_sub
      .apply_mediator_command(
        MediatorCommand::try_remove("fraktor://sys/user/actor", subscriber).expect("remove"),
        0,
        &[],
      )
      .expect("mediator command"),
    MediatorCommandOutcome::Noop
  );
}
