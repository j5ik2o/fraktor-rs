//! At-least-once delivery walkthrough.

extern crate alloc;

use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::SendError,
  messaging::AnyMessageGeneric,
};
use fraktor_persistence_rs::core::{AtLeastOnceDelivery, AtLeastOnceDeliveryConfig, RedeliveryTick};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, time::TimerInstant};

type TB = NoStdToolbox;

struct NoopSender;

impl ActorRefSender<TB> for NoopSender {
  fn send(&mut self, _message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    Ok(SendOutcome::Delivered)
  }
}

fn main() {
  // 日本語コメント: デフォルト設定でトラッカーを作成する
  let mut delivery: AtLeastOnceDelivery<TB> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());

  // 日本語コメント: 配送メッセージを送信し、確認する
  let now = TimerInstant::from_ticks(10, Duration::from_secs(1));
  let destination = ActorRefGeneric::new(Pid::new(1, 1), NoopSender);
  let delivery_id =
    delivery.deliver(destination.clone(), None, now, |id| (id, "payload")).expect("delivery should be accepted");
  let _confirmed = delivery.confirm_delivery(delivery_id);

  // 日本語コメント: RedeliveryTick の検出例
  let tick = RedeliveryTick;
  let _handled = delivery.handle_message(&tick, now);

  // 日本語コメント: スナップショットを取得して復元する
  let snapshot = delivery.get_delivery_snapshot();
  let mut restored: AtLeastOnceDelivery<TB> = AtLeastOnceDelivery::new(AtLeastOnceDeliveryConfig::default());
  restored.set_delivery_snapshot(snapshot, now);
}
