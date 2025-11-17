mod guardian;
mod lifecycle_printer;
mod printer;
mod start_message;

use std::{thread, time::Duration};

use fraktor_actor_std_rs::{
  event_stream::EventStreamSubscriber, messaging::AnyMessage, props::Props, system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use guardian::GuardianActor;
use lifecycle_printer::LifecyclePrinter;
use start_message::Start;

fn main() {
  let props = Props::from_fn(|| GuardianActor).with_name("named-guardian");
  let system = ActorSystem::new(&props).expect("ユーザーガーディアンの起動に失敗しました");

  let lifecycle_subscriber: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(LifecyclePrinter);
  let _subscription = system.subscribe_event_stream(&lifecycle_subscriber);

  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("Start メッセージの送信に失敗しました");

  thread::sleep(Duration::from_millis(50));

  system.terminate().expect("システムの停止に失敗しました");

  while !termination.is_ready() {
    thread::sleep(Duration::from_millis(10));
  }
}
