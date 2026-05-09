use core::time::Duration;
use std::{string::String, thread, time::Instant, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::{
  StdBlocker, dispatch::dispatcher::AffinityExecutor, tick_driver::StdTickDriver,
};
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    DEFAULT_DISPATCHER_ID, DefaultDispatcherFactory, DispatcherConfig, ExecutorShared, MessageDispatcherFactory,
    TrampolineState,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock, SpinSyncMutex};

const POOL_NAME: &str = "kernel-affinity-executor";
const WAIT_TIMEOUT: Duration = Duration::from_secs(3);

struct Greet;

struct GreeterActor {
  greetings: SharedLock<Vec<String>>,
}

impl Actor for GreeterActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Greet>().is_some() {
      // どのスレッド上で receive が走ったかを記録する。Inline executor 版では
      // 起動スレッド (StdTickDriver の exec_thread) でしか走らないが、ここでは
      // AffinityExecutor のワーカースレッド名 (`kernel-affinity-executor-N`) が観測される。
      let thread_name = thread::current().name().unwrap_or("<unnamed>").into();
      self.greetings.with_lock(|greetings| greetings.push(thread_name));
    }
    Ok(())
  }
}

fn main() {
  let greetings = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let greetings = greetings.clone();
    move || GreeterActor { greetings: greetings.clone() }
  });

  // `fraktor.actor.default-dispatcher` のデフォルトは Inline executor を指している。
  // ここでは AffinityExecutor (Pekko の `AffinityPool` 相当) を直接ぶら下げて
  // default-dispatcher を上書きする: 4 本のワーカースレッドと、各ワーカーが
  // 64 スロットの bounded queue を持つ。mailbox は `key % parallelism` で
  // 同じワーカーに固定されるため、同一アクターのメッセージは常に同じ OS
  // スレッドで処理される。
  let executor = ExecutorShared::new(Box::new(AffinityExecutor::new(POOL_NAME, 4, 64)), TrampolineState::new());
  let dispatcher_config = DispatcherConfig::with_defaults(DEFAULT_DISPATCHER_ID);
  let dispatcher_factory: ArcShared<Box<dyn MessageDispatcherFactory>> =
    ArcShared::new(Box::new(DefaultDispatcherFactory::new(&dispatcher_config, executor)));
  let actor_system_config =
    ActorSystemConfig::new(StdTickDriver::default()).with_dispatcher_factory(DEFAULT_DISPATCHER_ID, dispatcher_factory);

  let system = ActorSystem::create_from_props(&props, actor_system_config).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Greet));
  wait_until_deadline(Instant::now() + WAIT_TIMEOUT, || greetings.with_lock(|greetings| greetings.len() == 1));

  // receive は AffinityExecutor のワーカースレッド上で走るため、記録された
  // スレッド名はプール接頭辞で始まっているはず。Inline 版との挙動差を
  // この assert で固定する。
  let observed = greetings.with_lock(|greetings| greetings[0].clone());
  assert!(
    observed.starts_with(POOL_NAME),
    "Greet should be processed on a {POOL_NAME}-* worker thread, observed: {observed}"
  );
  println!("kernel_affinity_executor processed Greet on {observed}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until_deadline(deadline: Instant, mut condition: impl FnMut() -> bool) {
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
