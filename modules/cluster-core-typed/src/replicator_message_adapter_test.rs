use alloc::{collections::BTreeMap, string::ToString};
use core::time::Duration;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::actor::{extension::ExtensionInstallers, setup::ActorSystemConfig};
use fraktor_actor_core_typed_rs::{TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::NoopClusterProvider,
  ddata::{
    DeleteWriteOutcome, Flag, FlagKey, ReadConsistency, ReplicatorEntry, Subscribe, SubscribeResponse, Update,
    UpdateResponse, UpdateWriteOutcome, WriteConsistency,
  },
  extension::{ClusterExtensionConfig, ClusterExtensionInstaller},
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{ReplicatorCommand, UpdateModifyFn};

const ASK_TIMEOUT: Duration = Duration::from_secs(1);

struct LocalReplicator {
  entries: BTreeMap<alloc::string::String, ReplicatorEntry<Flag>>,
}

impl LocalReplicator {
  const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  fn entry_data(&self, key: &str) -> Option<&Flag> {
    match self.entries.get(key)? {
      | ReplicatorEntry::Present(data) => Some(data),
      | ReplicatorEntry::Missing | ReplicatorEntry::Deleted => None,
    }
  }

  fn handle(&mut self, command: &ReplicatorCommand<Flag>) {
    match command {
      | ReplicatorCommand::Get { command, reply_to } => {
        let entry = self.entries.get(command.key().id()).cloned().unwrap_or(ReplicatorEntry::Missing);
        let mut reply = reply_to.clone();
        let _ = reply.try_tell(command.respond_from(&entry));
      },
      | ReplicatorCommand::Update { command, modify, reply_to } => {
        let entry = self.entries.get(command.key().id()).cloned().unwrap_or(ReplicatorEntry::Missing);
        let (next, response) = command.evaluate(&entry, |current| modify.apply(current), UpdateWriteOutcome::Success);
        self.entries.insert(command.key().id().to_string(), next);
        let mut reply = reply_to.clone();
        let _ = reply.try_tell(response);
      },
      | ReplicatorCommand::Delete { command, reply_to } => {
        let entry = self.entries.get(command.key().id()).cloned().unwrap_or(ReplicatorEntry::Missing);
        let (next, response) = command.evaluate(&entry, DeleteWriteOutcome::Success);
        self.entries.insert(command.key().id().to_string(), next);
        let mut reply = reply_to.clone();
        let _ = reply.try_tell(response);
      },
      | ReplicatorCommand::Subscribe(command) => {
        let entry = self.entries.get(command.key().id()).cloned().unwrap_or(ReplicatorEntry::Missing);
        if let ReplicatorEntry::Present(data) = entry {
          let mut subscriber = command.subscriber().clone();
          let _ = subscriber.try_tell(command.changed(data));
        }
      },
      | ReplicatorCommand::Unsubscribe(_)
      | ReplicatorCommand::GetReplicaCount { .. }
      | ReplicatorCommand::FlushChanges(_) => {},
    }
  }
}

fn build_typed_system<M>() -> TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  let cluster_installer = ClusterExtensionInstaller::new(
    ClusterExtensionConfig::new().with_advertised_address("node1:8080"),
    |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new()),
  );
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_extension_installers(installers);
  let props = TypedProps::<M>::from_behavior_factory(Behaviors::ignore);
  TypedActorSystem::create_from_props(&props, config).expect("typed system")
}

#[test]
fn local_replicator_applies_update_and_notifies_subscriber() {
  let system = build_typed_system::<SubscribeResponse<Flag>>();
  let key = FlagKey::new("flag");
  let mut replicator = LocalReplicator::new();

  let update_reply = {
    let reply_props = TypedProps::<UpdateResponse<Flag>>::from_behavior_factory(Behaviors::ignore);
    let reply_actor = system.as_untyped().actor_of(reply_props.to_untyped()).expect("reply actor");
    TypedActorRef::<UpdateResponse<Flag>>::from_untyped(reply_actor.into_actor_ref())
  };
  replicator.handle(&ReplicatorCommand::update(
    Update::new(key.clone(), WriteConsistency::Local),
    UpdateModifyFn::new(|_| Ok(Flag::disabled().switch_on())),
    update_reply,
  ));
  assert!(replicator.entry_data(key.id()).expect("entry exists").is_enabled());

  let observed = ArcShared::new(SpinSyncMutex::new(None::<SubscribeResponse<Flag>>));
  let subscriber_props = TypedProps::<SubscribeResponse<Flag>>::from_behavior_factory({
    let observed = observed.clone();
    move || {
      let observed = observed.clone();
      Behaviors::receive_message(move |_ctx, event: &SubscribeResponse<Flag>| {
        *observed.lock() = Some(event.clone());
        Ok(Behaviors::same())
      })
    }
  });
  let subscriber_actor = system.as_untyped().actor_of(subscriber_props.to_untyped()).expect("spawn subscriber");
  let subscriber = TypedActorRef::<SubscribeResponse<Flag>>::from_untyped(subscriber_actor.into_actor_ref());
  replicator.handle(&ReplicatorCommand::subscribe(Subscribe::new(key, subscriber)));

  wait_until(|| observed.lock().is_some());
  assert!(observed.lock().as_ref().expect("event").data().expect("data").is_enabled());
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
  for _ in 0..200 {
    if predicate() {
      return;
    }
    std::thread::sleep(Duration::from_millis(5));
  }
  panic!("condition not met before timeout");
}
