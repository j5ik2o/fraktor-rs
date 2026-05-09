use fraktor_actor_core_kernel_rs::{
  actor::{ActorContext, Pid, error::ActorError, messaging::AnyMessage},
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::SharedAccess;

use crate::{
  TypedProps,
  actor::TypedActorContext,
  behavior::{Behavior, BehaviorDirective},
  dsl::{Behaviors, receive::Receive},
  message_and_signals::BehaviorSignal,
};

// --- ヘルパー ---------------------------------------------------------------

fn make_typed_ctx() -> (ActorSystem, Pid) {
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  (system, pid)
}

// --- Behaviors::receive は Receive<M> を返す ---------------------------------

#[test]
fn behaviors_receive_returns_receive_type() {
  // 前提: メッセージハンドラクロージャがある
  let handler =
    |_ctx: &mut TypedActorContext<'_, u32>, _msg: &u32| -> Result<Behavior<u32>, ActorError> { Ok(Behaviors::same()) };

  // 操作: Behaviors::receive を呼ぶ
  let receive: Receive<u32> = Behaviors::receive(handler);

  // 期待: Receive<u32> が得られる
  let _: Receive<u32> = receive;
}

// --- Receive::receive_signal は Behavior<M> へ連結される -----------------------

#[test]
fn receive_signal_chains_into_behavior() {
  // 前提: Behaviors::receive から得た Receive<u32> がある
  let receive: Receive<u32> = Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same()));

  // 操作: シグナルハンドラ付きで receive_signal を呼ぶ
  let behavior: Behavior<u32> = receive.receive_signal(|_ctx, _signal| Ok(Behaviors::same()));

  // 期待: signal handler を持つ Behavior<u32> になる
  assert!(behavior.has_signal_handler(), "chained behavior should have a signal handler");
}

// --- Receive<M> は From 経由で Behavior<M> に変換できる --------------------------

#[test]
fn receive_converts_to_behavior_via_from() {
  // 前提: Receive<u32> がある
  let receive: Receive<u32> = Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same()));

  // 操作: Into で Behavior<M> に変換する
  let behavior: Behavior<u32> = receive.into();

  // 期待: signal handler は付与されていない
  assert!(!behavior.has_signal_handler());
}

// --- Receive のメッセージハンドラが呼ばれる --------------------------

#[test]
fn receive_message_handler_is_invoked() {
  // 前提: 42 を受けると stopped を返す Receive<u32> がある
  let receive =
    Behaviors::receive(|_ctx, msg: &u32| if *msg == 42 { Ok(Behaviors::stopped()) } else { Ok(Behaviors::same()) });

  // 操作: Behavior に変換して 42 を処理する
  let mut behavior: Behavior<u32> = receive.into();
  let (system, pid) = make_typed_ctx();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let next = behavior.handle_message(&mut typed_ctx, &42).expect("message 42 should produce a next behavior");

  // 期待: Stopped を返す
  assert!(matches!(next.directive(), BehaviorDirective::Stopped));
}

// --- Receive に signal handler を追加できる --

#[test]
fn receive_with_signal_sets_signal_handler() {
  // 前提: signal handler を追加した Behavior<u32> がある
  let behavior: Behavior<u32> =
    Behaviors::receive(|_ctx, _msg: &u32| Ok(Behaviors::same())).receive_signal(|_ctx, signal| match signal {
      | BehaviorSignal::PostStop => Ok(Behaviors::stopped()),
      | _ => Ok(Behaviors::same()),
    });

  // 期待: signal handler が設定される
  assert!(behavior.has_signal_handler(), "behavior should have signal handler from chain");
}

// --- Behaviors::receive は既存の receive_message を壊さない -------------

#[test]
fn receive_message_still_works_independently() {
  // 前提: 既存の Behaviors::receive_message 呼び出しがある
  let behavior: Behavior<u32> = Behaviors::receive_message(|_ctx, _msg| Ok(Behaviors::same()));

  // 操作: Receive を経由せずに直接 Behavior として使う
  let (system, pid) = make_typed_ctx();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);
  let mut b = behavior;
  let result = b.handle_message(&mut typed_ctx, &99u32);

  // 期待: same ビヘイビアを返す
  let next = result.expect("receive_message handler should return a next behavior");
  assert!(matches!(next.directive(), BehaviorDirective::Same));
}

#[test]
fn receive_can_be_used_directly_in_typed_props_factory() {
  // 前提: Behaviors::receive から作った typed props がある
  let props = TypedProps::<u32>::from_behavior_factory(|| Behaviors::receive(|_ctx, _msg| Ok(Behaviors::same())));

  // 操作: untyped props 経由で保持された factory を実行する
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let pid = system.allocate_pid();
  let mut actor = props.to_untyped().factory().expect("typed props factory").with_write(|factory| factory.create());
  let mut context = ActorContext::new(&system, pid);

  // 期待: 生成された actor が正常にメッセージを処理できる
  let result = actor.receive(&mut context, AnyMessage::new(7_u32).as_view());
  assert!(result.is_ok(), "typed props factory should produce a runnable actor");
}
