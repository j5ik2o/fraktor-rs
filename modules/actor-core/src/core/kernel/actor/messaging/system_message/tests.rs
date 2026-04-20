use core::time::Duration;

use super::{FailurePayload, SystemMessage};
use crate::core::kernel::actor::{
  Pid,
  error::{ActorError, ActorErrorReason},
  messaging::{AnyMessage, Kill, PoisonPill},
};

#[test]
fn watch_message_round_trips_through_any_message() {
  let watcher = Pid::new(1, 0);
  let payload = SystemMessage::Watch(watcher);
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn create_message_round_trips_through_any_message() {
  let payload = SystemMessage::Create;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn poison_pill_message_round_trips_through_any_message() {
  let payload = SystemMessage::PoisonPill;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn poison_pill_public_message_converts_to_system_message() {
  // public PoisonPill が runtime の SystemMessage::PoisonPill へ正しくマッピングされることを保証する
  let converted = SystemMessage::from(PoisonPill);
  assert_eq!(converted, SystemMessage::PoisonPill);
}

#[test]
fn poison_pill_public_message_is_stored_as_distinct_payload_in_any_message() {
  // AnyMessage は public 型をそのまま保持し、SystemMessage へ暗黙変換しないことを保証する
  let stored = AnyMessage::new(PoisonPill);
  let view = stored.as_view();
  assert!(view.downcast_ref::<PoisonPill>().is_some());
  assert!(view.downcast_ref::<SystemMessage>().is_none());
}

#[test]
fn kill_message_round_trips_through_any_message() {
  let payload = SystemMessage::Kill;
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn kill_public_message_converts_to_system_message() {
  // public Kill が runtime の SystemMessage::Kill へ正しくマッピングされることを保証する
  let converted = SystemMessage::from(Kill);
  assert_eq!(converted, SystemMessage::Kill);
}

#[test]
fn kill_public_message_is_stored_as_distinct_payload_in_any_message() {
  // AnyMessage は public 型をそのまま保持し、SystemMessage へ暗黙変換しないことを保証する
  let stored = AnyMessage::new(Kill);
  let view = stored.as_view();
  assert!(view.downcast_ref::<Kill>().is_some());
  assert!(view.downcast_ref::<SystemMessage>().is_none());
}

#[test]
fn recreate_message_round_trips_through_any_message() {
  // AC-H4: `SystemMessage::Recreate` は `ActorErrorReason` ペイロードを同梱する。
  // Pekko `SystemMessage.scala` の `Recreate(cause: Throwable)` を参照し、
  // round-trip 時にも cause が保持されることを保証する。
  let cause = ActorErrorReason::new("ac-h4-recreate-round-trip");
  let payload = SystemMessage::Recreate(cause.clone());
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
  match recovered {
    | SystemMessage::Recreate(restored) => assert_eq!(restored, &cause),
    | other => panic!("expected SystemMessage::Recreate, got {other:?}"),
  }
}

#[test]
fn failure_message_round_trips_through_any_message() {
  let reason = ActorError::recoverable("boom");
  let failure = FailurePayload::from_error(Pid::new(5, 0), &reason, None, Duration::from_millis(1));
  let payload = SystemMessage::Failure(failure);
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
}

#[test]
fn failure_payload_to_actor_error_preserves_classification() {
  let recoverable =
    FailurePayload::from_error(Pid::new(10, 0), &ActorError::recoverable("ok"), None, Duration::from_secs(1));
  let fatal = FailurePayload::from_error(Pid::new(11, 0), &ActorError::fatal("bad"), None, Duration::from_secs(2));
  let escalate =
    FailurePayload::from_error(Pid::new(12, 0), &ActorError::escalate("boom"), None, Duration::from_secs(3));

  if let ActorError::Recoverable(_) = recoverable.to_actor_error() {
  } else {
    panic!("expected recoverable");
  }

  if let ActorError::Fatal(_) = fatal.to_actor_error() {
  } else {
    panic!("expected fatal");
  }

  // SP-H1 regression: Escalate が ActorError::Escalate として round-trip し、
  // supervisor 再構成経路で Fatal に潰れて default decider により停止されず、
  // 親へ escalation できることを保証する。
  if let ActorError::Escalate(_) = escalate.to_actor_error() {
  } else {
    panic!("expected escalate");
  }
}

#[test]
fn death_watch_notification_carries_pid() {
  // AC-H5: `SystemMessage::Terminated(Pid)` variant は削除済みで、kernel 内の
  // watcher 通知は `DeathWatchNotification(Pid)` のみが担う。
  let target = Pid::new(7, 0);
  if let SystemMessage::DeathWatchNotification(pid) = SystemMessage::DeathWatchNotification(target) {
    assert_eq!(pid, target);
  } else {
    panic!("unexpected variant");
  }
}

#[test]
fn death_watch_notification_round_trips_through_any_message() {
  // AC-H5: Pekko `DeathWatch.scala` の `DeathWatchNotification(actor)` に相当する
  // 新 variant `SystemMessage::DeathWatchNotification(Pid)` が AnyMessage 経由で
  // round-trip 可能であることを保証する。kernel→kernel の watcher 通知に使用され、
  // user-queue 上の `Terminated` (= 公開 user-level メッセージ) とは別のチャネル。
  let target = Pid::new(50, 0);
  let payload = SystemMessage::DeathWatchNotification(target);
  let stored: AnyMessage = payload.clone().into();
  let view = stored.as_view();
  let recovered = view.downcast_ref::<SystemMessage>().expect("system message");
  assert_eq!(recovered, &payload);
  match recovered {
    | SystemMessage::DeathWatchNotification(pid) => assert_eq!(pid, &target),
    | other => panic!("expected SystemMessage::DeathWatchNotification, got {other:?}"),
  }
}

#[test]
fn death_watch_notification_is_distinct_from_watch() {
  // AC-H5: `DeathWatchNotification(Pid)` は kernel 内の termination 通知専用で、
  // `Watch(Pid)` (subscribe 要求) とは別 variant。本 change で
  // `SystemMessage::Terminated(Pid)` は enum から削除済み（kernel 内送信元が
  // 消えたため未使用 variant を残さない原則に従う）。
  let target = Pid::new(51, 0);
  let dwn = SystemMessage::DeathWatchNotification(target);
  let watch = SystemMessage::Watch(target);
  assert_ne!(dwn, watch, "DeathWatchNotification と Watch は別 variant");
}
