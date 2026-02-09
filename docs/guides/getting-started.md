# Getting Started

fraktor-rs は Apache Pekko / Proto.Actor のセマンティクスを Rust に移植したアクターランタイムです。`no_std`（組込み）環境と `std`（Tokio 連携）環境の双方で同一の API を提供し、ガーディアンアクターを起点とした階層的なアクターモデルを構築できます。

## 1. 依存関係の追加

### std 環境（Tokio 連携）

```toml
[dependencies]
fraktor-actor-rs = { version = "0.2", features = ["std"] }
fraktor-utils-rs = { version = "0.2", features = ["std"] }
```

Tokio の Dispatcher を利用する場合は `tokio-executor` feature を有効にします。

```toml
[dependencies]
fraktor-actor-rs = { version = "0.2", features = ["tokio-executor"] }
fraktor-utils-rs = { version = "0.2", features = ["std"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

### no_std 環境

```toml
[dependencies]
fraktor-actor-rs = { version = "0.2", default-features = false }
fraktor-utils-rs = { version = "0.2", default-features = false, features = ["alloc"] }
```

`no_std` 環境では `alloc` クレートが必要です。ヒープアロケータの設定はターゲットに応じて別途行ってください。

## 2. 基本概念

fraktor-rs のアクターモデルは以下の要素で構成されます。

| 概念 | 説明 |
|------|------|
| **ActorSystem** | アクターランタイムのルート。ガーディアンアクターを保持する |
| **Guardian** | ActorSystem 直下のトップレベルアクター。子アクターを生成する起点 |
| **Props** | アクターの生成方法を記述するファクトリ |
| **Actor trait** | `receive(&mut self, ctx, message)` でメッセージを処理する Untyped API |
| **TypedActor trait** | 型付きメッセージを受け取る Typed API |
| **TickDriver** | スケジューラのタイマー駆動を提供する仕組み |

## 3. 最小サンプル（std / Tokio 版）

Tokio ランタイム上で Ping-Pong メッセージングを行う最小例です。

```rust
use std::time::Duration;

use fraktor_actor_rs::{
  core::error::ActorError,
  std::{
    actor::{Actor, ActorContext, ActorRef},
    dispatch::dispatcher::dispatch_executor::TokioExecutor,
    futures::ActorFutureListener,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    system::{ActorSystem, DispatcherConfig},
  },
};
use fraktor_utils_rs::{core::sync::ArcShared, std::StdSyncMutex};
use tokio::runtime::Handle;

// --- メッセージ型 ---
struct Start;
struct Ping { reply_to: ActorRef }
struct Pong;

// --- Guardian ---
struct GuardianActor { dispatcher: DispatcherConfig }

impl Actor for GuardianActor {
  fn receive(
    &mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong = ctx.spawn_child(
        &Props::from_fn(|| PongActor).with_dispatcher(self.dispatcher.clone()),
      ).map_err(|_| ActorError::recoverable("spawn failed"))?;

      let mut ping = ctx.spawn_child(
        &Props::from_fn(|| PingActor).with_dispatcher(self.dispatcher.clone()),
      ).map_err(|_| ActorError::recoverable("spawn failed"))?;

      ping.tell(AnyMessage::new(Ping { reply_to: pong.actor_ref().clone() }))
        .map_err(|_| ActorError::recoverable("tell failed"))?;
    } else if message.downcast_ref::<Pong>().is_some() {
      println!("Pong received!");
    }
    Ok(())
  }
}

// --- Ping Actor ---
struct PingActor;

impl Actor for PingActor {
  fn receive(
    &mut self, ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>,
  ) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      println!("Sending ping...");
      ping.reply_to.clone()
        .tell(AnyMessage::new(Pong))
        .map_err(|_| ActorError::recoverable("tell failed"))?;
    }
    Ok(())
  }
}

// --- Pong Actor ---
struct PongActor;

impl Actor for PongActor {
  fn receive(
    &mut self, _ctx: &mut ActorContext<'_, '_>, message: AnyMessageView<'_>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Pong>().is_some() {
      println!("Pong actor received pong");
    }
    Ok(())
  }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let handle = Handle::current();
  let dispatcher: DispatcherConfig =
    DispatcherConfig::from_executor(
      ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle)))),
    );

  let props = Props::from_fn({
    let d = dispatcher.clone();
    move || GuardianActor { dispatcher: d.clone() }
  }).with_dispatcher(dispatcher);

  let tick_driver = fraktor_actor_rs::std::scheduler::tick::TickDriverConfig::tokio_quickstart();
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  tokio::time::sleep(Duration::from_millis(50)).await;
  system.terminate().expect("terminate");
  ActorFutureListener::new(system.when_terminated()).await;
}
```

## 4. 最小サンプル（no_std 版）

`no_std` 環境（ホスト上でのシミュレーション）で同様の Ping-Pong を行う例です。

```rust
#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

use alloc::string::String;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContext, actor_ref::ActorRef},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::SharedAccess;

struct Start;
struct Ping { reply_to: ActorRef }
struct Pong;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(
    &mut self, ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      let pong = ctx.spawn_child(&Props::from_fn(|| PongActor))
        .map_err(|_| ActorError::recoverable("spawn failed"))?;
      let mut ping = ctx.spawn_child(&Props::from_fn(|| PingActor))
        .map_err(|_| ActorError::recoverable("spawn failed"))?;
      ping.tell(AnyMessage::new(Ping { reply_to: pong.actor_ref().clone() }))
        .map_err(|_| ActorError::recoverable("tell failed"))?;
    } else if message.downcast_ref::<Pong>().is_some() {
      println!("Pong received!");
    }
    Ok(())
  }
}

struct PingActor;

impl Actor for PingActor {
  fn receive(
    &mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>,
  ) -> Result<(), ActorError> {
    if let Some(ping) = message.downcast_ref::<Ping>() {
      ping.reply_to.clone()
        .tell(AnyMessage::new(Pong))
        .map_err(|_| ActorError::recoverable("tell failed"))?;
    }
    Ok(())
  }
}

struct PongActor;

impl Actor for PongActor {
  fn receive(
    &mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageViewGeneric<'_>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Pong>().is_some() {
      println!("Pong!");
    }
    Ok(())
  }
}

// TickDriver の設定は省略（実際には TickDriverConfig の構成が必要）
// 完全な例は modules/actor/examples/ping_pong_not_std/main.rs を参照してください

#[cfg(not(target_os = "none"))]
fn main() {
  use std::thread;

  // no_std 向け TickDriver の構成（デモ用ハードウェアパルス）
  // 詳細は modules/actor/examples/no_std_tick_driver_support.rs を参照
  let props = Props::from_fn(|| GuardianActor);
  // let (tick_driver, _pulse_handle) = hardware_tick_driver_config();
  // let system = ActorSystem::new(&props, tick_driver).expect("system");
  // let termination = system.when_terminated();
  // system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
  // system.terminate().expect("terminate");
  // while !termination.with_read(|af| af.is_ready()) {
  //   thread::yield_now();
  // }
}
```

> **注意**: no_std 環境では `TickDriverConfig` の構成が必要です。完全な動作例は `modules/actor/examples/ping_pong_not_std/` を参照してください。

## 5. サンプルの実行方法

### Tokio 版（std）

```bash
cargo run -p fraktor-actor-rs --example ping_pong_tokio_std --features tokio-executor
```

期待される出力:

```
[ThreadId(N)] received ping: ping-1
[ThreadId(N)] pong replied: ping-1
[ThreadId(N)] received ping: ping-2
[ThreadId(N)] pong replied: ping-2
[ThreadId(N)] received ping: ping-3
[ThreadId(N)] pong replied: ping-3
```

### no_std 版（ホスト上で実行）

```bash
cargo run -p fraktor-actor-rs --example ping_pong_not_std
```

期待される出力の内容（ping/pong が3往復すること）は Tokio 版と同様ですが、実行環境の違いにより表示順は前後する場合があります。

### Typed Actor 版（no_std）

```bash
cargo run -p fraktor-actor-rs --example ping_pong_typed_not_std
```

### Behavior ベースのカウンター（std）

```bash
cargo run -p fraktor-actor-rs --example behaviors_counter_typed_std --features std
```

### Supervision（std）

```bash
cargo run -p fraktor-actor-rs --example supervision_std --features std
```

## 6. 次のステップ

基本的な Ping-Pong を動かせたら、以下のトピックに進んでください。

| トピック | 参照先 |
|----------|--------|
| **Typed Actor** | `examples/ping_pong_typed_not_std` - 型付きメッセージによる安全な通信 |
| **Behavior パターン** | `examples/behaviors_counter_typed_std` - 関数型スタイルのアクター定義 |
| **Supervision（監督）** | `examples/supervision_std` - 子アクターの障害復旧戦略 |
| **DeathWatch** | `examples/death_watch_std` - アクター終了の監視 |
| **Scheduler** | `examples/scheduler_once_no_std` - 遅延実行・周期実行 |
| **EventStream** | `examples/logger_subscriber_std` - ライフサイクルイベントの購読 |
| **Serialization** | `examples/serialization_json_std` - メッセージのシリアライズ |
| **Remoting** | `modules/remote/examples/loopback_quickstart` - プロセス間通信 |
| **Cluster** | `modules/cluster/examples/quickstart` - 複数ノードのクラスタリング |
| **Persistence** | `modules/persistence/examples/persistent_counter_no_std` - イベントソーシング |
| **Streams** | `modules/streams/examples/actor_system_basic_std` - ストリーム処理 |

### 関連ガイド

- [ActorSystem ガイド](actor-system.md) - システムの初期化・メッセージ送信・停止フローの詳細
- [TickDriver クイックスタート](tick-driver-quickstart.md) - スケジューラのタイマー駆動の設定方法
- [Remoting クイックスタート](remoting-quickstart.md) - リモートアクター通信の設定方法
- [Shared vs Handle](shared_vs_handle.md) - 共有ラッパーとハンドルの命名・分離ガイド
- [Persistence クイックスタート](persistence-quickstart.md) - イベントソーシングによるアクター永続化
