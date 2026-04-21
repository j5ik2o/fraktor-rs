use alloc::vec::Vec;
use core::hint::spin_loop;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  kernel::actor::{scheduler::tick_driver::tests::TestTickDriver, setup::ActorSystemConfig},
  typed::{
    TypedActorRef, TypedActorSystem, TypedProps,
    dsl::Behaviors,
    pubsub::{Topic, TopicCommand, TopicStats},
  },
};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

#[test]
fn topic_should_publish_to_subscribers_and_report_stats() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::create_with_config(&guardian_props, ActorSystemConfig::new(TestTickDriver::default()))
      .expect("system");

  let topic_props = TypedProps::<TopicCommand<u32>>::from_behavior_factory(|| Topic::behavior("numbers"));
  let topic = system.as_untyped().spawn(topic_props.to_untyped()).expect("spawn topic");
  let mut topic = TypedActorRef::<TopicCommand<u32>>::from_untyped(topic.into_actor_ref());

  let received = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber_props = TypedProps::<u32>::from_behavior_factory({
    let received = received.clone();
    move || {
      let received = received.clone();
      Behaviors::receive_message(move |_ctx, message: &u32| {
        received.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let subscriber = system.as_untyped().spawn(subscriber_props.to_untyped()).expect("spawn subscriber");
  let subscriber_ref = TypedActorRef::<u32>::from_untyped(subscriber.into_actor_ref());

  topic.tell(Topic::subscribe(subscriber_ref.clone()));
  topic.tell(Topic::publish(42_u32));
  wait_until(|| received.lock().as_slice() == [42_u32]);

  let stats = topic.ask::<TopicStats, _>(Topic::get_topic_stats);
  wait_until(|| stats.future().is_ready());
  let mut stats_future = stats.future().clone();
  let stats = stats_future.try_take().expect("stats ready").expect("stats ok");
  assert_eq!(stats.local_subscriber_count(), 1);
  assert!(stats.topic_instance_count() >= 1);

  topic.tell(Topic::unsubscribe(subscriber_ref));
  topic.tell(Topic::publish(99_u32));
  wait_until(|| received.lock().as_slice() == [42_u32]);

  system.terminate().expect("terminate");
}
