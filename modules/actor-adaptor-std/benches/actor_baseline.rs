use std::{
  boxed::Box,
  hint::black_box,
  sync::mpsc::{Receiver, SyncSender, sync_channel},
  time::Duration,
};

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use fraktor_actor_adaptor_std_rs::std::{default_tick_driver_config, dispatch::dispatcher::TokioExecutor};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::{
    dispatcher::{
      DEFAULT_DISPATCHER_ID, DefaultDispatcherConfigurator, DispatcherSettings, ExecutorShared,
      MessageDispatcherConfigurator,
    },
    mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy},
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::runtime::{Builder, Runtime};

const WAIT_TIMEOUT: Duration = Duration::from_secs(1);

struct SpawnOnce {
  done: SyncSender<()>,
}

struct RegisterChild {
  reply_to: SyncSender<ActorRef>,
}

struct Notify {
  done: SyncSender<()>,
}

struct PingPong {
  done:      SyncSender<()>,
  peer:      ActorRef,
  remaining: usize,
}

struct SilentActor;

impl Actor for SilentActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct SpawnGuardian;

impl Actor for SpawnGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<SpawnOnce>() {
      let child =
        ctx.spawn_child(&Props::from_fn(|| SilentActor)).map_err(|_| ActorError::recoverable("spawn failed"))?;
      black_box(child);
      command.done.send(()).expect("spawn bench ack");
    }
    Ok(())
  }
}

struct NotifyActor;

impl Actor for NotifyActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<Notify>() {
      command.done.send(()).expect("notify bench ack");
    }
    Ok(())
  }
}

struct RegistryGuardian;

impl Actor for RegistryGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<RegisterChild>() {
      let child = ctx
        .spawn_child(&Props::from_fn(|| NotifyActor))
        .map_err(|_| ActorError::recoverable("spawn notify child failed"))?;
      command.reply_to.send(child.into_actor_ref()).expect("register child ref");
    }
    Ok(())
  }
}

struct PingPongRegistryGuardian;

impl Actor for PingPongRegistryGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<RegisterChild>() {
      let child = ctx
        .spawn_child(&Props::from_fn(|| PingPongActor))
        .map_err(|_| ActorError::recoverable("spawn ping-pong child failed"))?;
      command.reply_to.send(child.into_actor_ref()).expect("register ping-pong child ref");
    }
    Ok(())
  }
}

struct PingPongActor;

impl Actor for PingPongActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<PingPong>() {
      if command.remaining == 0 {
        command.done.send(()).expect("ping-pong bench ack");
      } else {
        let next =
          PingPong { done: command.done.clone(), peer: ctx.self_ref(), remaining: command.remaining - 1 };
        command.peer.clone().tell(AnyMessage::new(next));
      }
    }
    Ok(())
  }
}

struct TokioBenchSystem {
  runtime: Runtime,
  system:  ActorSystem,
}

impl TokioBenchSystem {
  fn new(props: &Props) -> Self {
    let runtime = Builder::new_multi_thread().worker_threads(2).enable_time().build().expect("tokio runtime");
    let handle = runtime.handle().clone();
    let system = runtime.block_on(async {
      let settings = DispatcherSettings::with_defaults(DEFAULT_DISPATCHER_ID);
      let executor = ExecutorShared::new_with_builtin_lock(TokioExecutor::new(handle));
      let configurator: Box<dyn MessageDispatcherConfigurator> =
        Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
      let config = ActorSystemConfig::default()
        .with_tick_driver(default_tick_driver_config())
        .with_dispatcher_configurator(DEFAULT_DISPATCHER_ID, ArcShared::new(configurator));
      ActorSystem::new_with_config(props, &config).expect("actor system")
    });
    Self { runtime, system }
  }

  fn terminate(self) {
    self.runtime.block_on(async {
      self.system.terminate().expect("terminate system");
      self.system.when_terminated().await;
    });
  }
}

struct SpawnBenchFixture {
  system: TokioBenchSystem,
}

impl SpawnBenchFixture {
  fn new() -> Self {
    let props = Props::from_fn(|| SpawnGuardian);
    Self { system: TokioBenchSystem::new(&props) }
  }

  fn spawn_once(&self) {
    let receiver = send_and_receive(|done| SpawnOnce { done }, self.system.system.user_guardian_ref());
    wait_for(receiver);
  }

  fn terminate(self) {
    self.system.terminate();
  }
}

struct TellBenchFixture {
  actor_ref: ActorRef,
  system:    TokioBenchSystem,
}

impl TellBenchFixture {
  fn new() -> Self {
    let props = Props::from_fn(|| RegistryGuardian);
    let system = TokioBenchSystem::new(&props);
    let (reply_to, receiver) = sync_channel(1);
    system.system.user_guardian_ref().tell(AnyMessage::new(RegisterChild { reply_to }));
    let actor_ref = receiver.recv_timeout(WAIT_TIMEOUT).expect("registered child ref");
    Self { actor_ref, system }
  }

  fn tell_once(&self) {
    let receiver = send_and_receive(|done| Notify { done }, self.actor_ref.clone());
    wait_for(receiver);
  }

  fn terminate(self) {
    self.system.terminate();
  }
}

struct PingPongBenchFixture {
  ping_ref: ActorRef,
  pong_ref: ActorRef,
  system:   TokioBenchSystem,
}

impl PingPongBenchFixture {
  fn new() -> Self {
    let props = Props::from_fn(|| PingPongRegistryGuardian);
    let system = TokioBenchSystem::new(&props);
    let ping_ref = register_child(&system.system);
    let pong_ref = register_child(&system.system);
    Self { ping_ref, pong_ref, system }
  }

  fn run_roundtrip(&self, rounds: usize) {
    let (done, receiver) = sync_channel(1);
    let message = PingPong { done, peer: self.pong_ref.clone(), remaining: rounds };
    self.ping_ref.clone().tell(AnyMessage::new(message));
    wait_for(receiver);
  }

  fn terminate(self) {
    self.system.terminate();
  }
}

fn send_and_receive<T, F>(message_factory: F, mut target: ActorRef) -> Receiver<()>
where
  F: FnOnce(SyncSender<()>) -> T,
  T: Send + Sync + 'static, {
  let (done, receiver) = sync_channel(1);
  target.tell(AnyMessage::new(message_factory(done)));
  receiver
}

fn wait_for(receiver: Receiver<()>) {
  receiver.recv_timeout(WAIT_TIMEOUT).expect("benchmark completion");
}

fn register_child(system: &ActorSystem) -> ActorRef {
  let (reply_to, receiver) = sync_channel(1);
  system.user_guardian_ref().tell(AnyMessage::new(RegisterChild { reply_to }));
  receiver.recv_timeout(WAIT_TIMEOUT).expect("registered child ref")
}

fn bench_spawn(c: &mut Criterion) {
  let mut group = c.benchmark_group("actor_spawn");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));
  group.bench_function("spawn_child", |b| {
    b.iter_batched(
      SpawnBenchFixture::new,
      |fixture| {
        fixture.spawn_once();
        fixture.terminate();
      },
      BatchSize::SmallInput,
    );
  });
  group.finish();
}

fn bench_tell(c: &mut Criterion) {
  let mut group = c.benchmark_group("actor_tell");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));
  group.bench_function("single_tell", |b| {
    let fixture = TellBenchFixture::new();
    b.iter(|| fixture.tell_once());
    fixture.terminate();
  });
  group.finish();
}

fn bench_ping_pong(c: &mut Criterion) {
  let mut group = c.benchmark_group("actor_ping_pong");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));

  for rounds in [100_usize, 1_000_usize] {
    group.throughput(Throughput::Elements(rounds as u64));
    group.bench_with_input(format!("roundtrip_{rounds}"), &rounds, |b, &rounds| {
      let fixture = PingPongBenchFixture::new();
      b.iter(|| fixture.run_roundtrip(rounds));
      fixture.terminate();
    });
  }

  group.finish();
}

fn bench_mailbox(c: &mut Criterion) {
  use core::num::NonZeroUsize;

  let mut group = c.benchmark_group("mailbox_enqueue");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));

  for capacity in [1_usize, 64_usize] {
    group.throughput(Throughput::Elements(capacity as u64));
    group.bench_with_input(format!("bounded_capacity_{capacity}"), &capacity, |b, &capacity| {
      b.iter_batched(
        || {
          Mailbox::new(MailboxPolicy::bounded(
            NonZeroUsize::new(capacity).expect("non-zero capacity"),
            MailboxOverflowStrategy::DropNewest,
            None,
          ))
        },
        |mailbox| {
          for sequence in 0..capacity {
            mailbox.enqueue_user(AnyMessage::new(sequence)).expect("enqueue benchmark message");
          }
          black_box(mailbox);
        },
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

criterion_group!(actor_baseline, bench_spawn, bench_tell, bench_ping_pong, bench_mailbox);
criterion_main!(actor_baseline);
