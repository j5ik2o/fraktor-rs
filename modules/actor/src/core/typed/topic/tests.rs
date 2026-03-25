use alloc::vec::Vec;
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, Topic, TopicCommand, TopicStats, TypedActorSystem, TypedProps, actor::TypedActorRef,
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
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let topic_props = TypedProps::<TopicCommand<u32>>::from_behavior_factory(|| Topic::behavior("numbers"));
  let topic = system.as_untyped().spawn(topic_props.to_untyped()).expect("spawn topic");
  let mut topic = TypedActorRef::<TopicCommand<u32>>::from_untyped(topic.actor_ref().clone());

  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
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
  let subscriber_ref = TypedActorRef::<u32>::from_untyped(subscriber.actor_ref().clone());

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
