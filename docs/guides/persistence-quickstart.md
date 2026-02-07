# Persistence クイックスタート

fraktor-rs の Persistence モジュールは、アクターの状態をイベントソーシングで永続化する仕組みを提供します。Apache Pekko の `EventSourcedBehavior` に相当する機能を `no_std` / `std` 双方で利用できます。

## 1. 概要

イベントソーシングでは、アクターの状態変更を **イベント** として Journal に記録し、再起動時にイベントを再生して状態を復元します。オプションとして **スナップショット** を保存すれば、再生するイベント数を削減できます。

```
コマンド受信 → イベント生成 → Journal に永続化 → ハンドラで状態更新
                                                ↓
                              再起動時: Journal からイベント再生 → 状態復元
```

## 2. Pekko との概念対応表

| Pekko (Scala) | fraktor-rs (Rust) | 説明 |
|----------------|-------------------|------|
| `EventSourcedBehavior` | `Eventsourced<TB>` trait | イベントソーシングのコアインターフェース |
| `PersistentActor` | `PersistentActor<TB>` trait | `Eventsourced` を拡張し、persist / snapshot 操作を提供 |
| `PersistenceId` | `persistence_id() -> &str` | アクターの永続化識別子 |
| `commandHandler` | `receive_command(&mut self, ctx, message)` | コマンド処理ハンドラ |
| `eventHandler` | `receive_recover(&mut self, repr)` | イベント再生ハンドラ |
| `snapshotHandler` | `receive_snapshot(&mut self, snapshot)` | スナップショット復元ハンドラ |
| `Recovery` | `Recovery` 構造体 | リカバリ設定（スナップショット条件、再生範囲） |
| `EventEnvelope` | `PersistentRepr` | 永続化イベントの表現（ペイロード + メタデータ） |
| `SnapshotMetadata` | `SnapshotMetadata` | スナップショットのメタデータ |
| `SnapshotSelectionCriteria` | `SnapshotSelectionCriteria` | スナップショット選択条件 |
| `EventJournal` (plugin) | `Journal` trait | イベントの読み書きインターフェース |
| `SnapshotStore` (plugin) | `SnapshotStore` trait | スナップショットの保存・読込インターフェース |
| `InmemJournal` | `InMemoryJournal` | テスト用インメモリ Journal |
| `InmemSnapshotStore` | `InMemorySnapshotStore` | テスト用インメモリ SnapshotStore |
| `PersistenceContext` | `PersistenceContext<A, TB>` | アクターが保持する永続化コンテキスト |
| Extension 登録 | `PersistenceExtensionInstaller` | ActorSystem への永続化拡張の組込み |

## 3. 最小限の永続アクター実装手順

カウンターアクターを例に、イベントソーシングの実装手順を説明します。

### ステップ 1: メッセージとイベントの定義

```rust
/// コマンド: アクターへの要求
#[derive(Clone)]
enum Command {
    Add(i32),
}

/// イベント: 状態変更の記録
#[derive(Clone)]
enum Event {
    Incremented(i32),
}
```

コマンドは「何をしてほしいか」、イベントは「何が起きたか」を表します。

### ステップ 2: アクター構造体の定義

```rust
use fraktor_persistence_rs::core::PersistenceContext;
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

type TB = NoStdToolbox;

struct CounterActor {
    context: PersistenceContext<CounterActor, TB>,
    value:   i32,
}

impl CounterActor {
    fn new(persistence_id: &str) -> Self {
        Self {
            context: PersistenceContext::new(persistence_id.into()),
            value: 0,
        }
    }

    fn apply_event(&mut self, event: &Event) {
        let Event::Incremented(delta) = event;
        self.value += delta;
    }
}
```

- `PersistenceContext<Self, TB>` をフィールドとして保持します
- `apply_event` はコマンド処理とリカバリの両方で使い回す状態更新メソッドです

### ステップ 3: Eventsourced trait の実装

```rust
use fraktor_persistence_rs::core::{Eventsourced, PersistentRepr, Snapshot};
use fraktor_actor_rs::core::{
    actor::ActorContextGeneric,
    error::ActorError,
    messaging::AnyMessageViewGeneric,
};

impl Eventsourced<TB> for CounterActor {
    fn persistence_id(&self) -> &str {
        self.context.persistence_id()
    }

    fn receive_recover(&mut self, repr: &PersistentRepr) {
        if let Some(event) = repr.downcast_ref::<Event>() {
            self.apply_event(event);
        }
    }

    fn receive_snapshot(&mut self, snapshot: &Snapshot) {
        if let Some(value) = snapshot.data().downcast_ref::<i32>() {
            self.value = *value;
        }
    }

    fn receive_command(
        &mut self,
        ctx: &mut ActorContextGeneric<'_, TB>,
        message: AnyMessageViewGeneric<'_, TB>,
    ) -> Result<(), ActorError> {
        if let Some(Command::Add(delta)) = message.downcast_ref::<Command>() {
            self.persist(ctx, Event::Incremented(*delta), |actor, event| {
                actor.apply_event(event);
            });
            self.flush_batch(ctx);
        }
        Ok(())
    }

    fn last_sequence_nr(&self) -> u64 {
        self.context.last_sequence_nr()
    }
}
```

各メソッドの役割:

| メソッド | 呼ばれるタイミング | 役割 |
|----------|-------------------|------|
| `persistence_id` | 常時 | 永続化ストアのキーとなる一意の識別子を返す |
| `receive_recover` | 再起動時 | Journal から再生されたイベントを適用して状態を復元する |
| `receive_snapshot` | 再起動時 | スナップショットから状態を復元する |
| `receive_command` | 通常稼働時 | コマンドを処理し、`persist` でイベントを永続化する |
| `last_sequence_nr` | 常時 | 現在のシーケンス番号を返す |

### ステップ 4: PersistentActor trait の実装

```rust
use fraktor_persistence_rs::core::PersistentActor;

impl PersistentActor<TB> for CounterActor {
    fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
        &mut self.context
    }
}
```

`PersistentActor` は `Eventsourced` を拡張し、`persist` / `flush_batch` / `save_snapshot` / `delete_messages` などの操作メソッドをデフォルト実装で提供します。ユーザーが実装するのは `persistence_context` のみです。

### ステップ 5: Guardian から永続アクターを生成

```rust
use fraktor_persistence_rs::core::{persistent_props, spawn_persistent};
use fraktor_actor_rs::core::{
    actor::Actor,
    messaging::AnyMessage,
    props::Props,
};

struct Start;

struct GuardianActor;

impl Actor<TB> for GuardianActor {
    fn receive(
        &mut self,
        ctx: &mut ActorContextGeneric<'_, TB>,
        message: AnyMessageViewGeneric<'_, TB>,
    ) -> Result<(), ActorError> {
        if message.downcast_ref::<Start>().is_none() {
            return Ok(());
        }

        // persistent_props で永続アクター用の Props を生成
        let props = persistent_props(|| CounterActor::new("counter-1"));
        // spawn_persistent で子アクターとして生成
        let child = spawn_persistent(ctx, &props)
            .map_err(|e| ActorError::recoverable(
                format!("spawn persistent actor failed: {e:?}")
            ))?;
        child.tell(AnyMessage::new(Command::Add(1)))
            .map_err(|_| ActorError::recoverable("send command failed"))?;
        Ok(())
    }
}
```

- `persistent_props(factory)`: `PersistentActorAdapter` でラップした `Props` を構築します
- `spawn_persistent(ctx, props)`: 永続アクターを子として生成し `ActorRef` を返します

### ステップ 6: ActorSystem に永続化拡張を登録

```rust
use fraktor_persistence_rs::core::{
    InMemoryJournal, InMemorySnapshotStore,
    PersistenceExtensionInstaller,
};
use fraktor_actor_rs::core::{
    extension::ExtensionInstallers,
    system::{ActorSystem, ActorSystemConfig},
};

fn main() {
    // Journal と SnapshotStore のインスタンスを作成
    let installer = PersistenceExtensionInstaller::new(
        InMemoryJournal::new(),
        InMemorySnapshotStore::new(),
    );

    // ExtensionInstallers に永続化拡張を追加
    let installers = ExtensionInstallers::default()
        .with_extension_installer(installer);

    let props = Props::from_fn(|| GuardianActor);
    let config = ActorSystemConfig::default()
        .with_tick_driver(tick_driver)          // TickDriver の設定
        .with_extension_installers(installers); // 永続化拡張を登録

    let system = ActorSystem::new_with_config(&props, &config)
        .expect("system");
    system.user_guardian_ref()
        .tell(AnyMessage::new(Start))
        .expect("start");
}
```

`PersistenceExtensionInstaller::new(journal, snapshot_store)` に実装を渡すことで、ActorSystem 起動時に Journal アクターと Snapshot アクターが自動的に生成されます。

## 4. persist / flush の流れ

`receive_command` 内でのイベント永続化は以下の順序で行います。

```
1. self.persist(ctx, event, handler)  -- イベントをバッチに積む
2. self.flush_batch(ctx)              -- バッチを Journal に送信
3. Journal が書込み完了を通知
4. handler が呼ばれ、状態を更新
```

- `persist` はイベントを内部バッチに追加するだけで、即座に永続化されません
- `flush_batch` を呼ぶことで、バッチ内の全イベントが Journal に送信されます
- 複数イベントをまとめて永続化する場合は `persist_all` を使います

## 5. スナップショットの利用

イベント数が多い場合、定期的にスナップショットを保存してリカバリ時間を短縮できます。

```rust
// コマンド処理内でスナップショットを保存
fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
) -> Result<(), ActorError> {
    if let Some(Command::Add(delta)) = message.downcast_ref::<Command>() {
        self.persist(ctx, Event::Incremented(*delta), |actor, event| {
            actor.apply_event(event);
        });
        self.flush_batch(ctx);

        // 100 イベントごとにスナップショットを保存
        if self.last_sequence_nr() % 100 == 0 {
            let snapshot = ArcShared::new(self.value);
            self.save_snapshot(ctx, snapshot);
        }
    }
    Ok(())
}
```

リカバリ時の動作:
1. 最新のスナップショットがあれば `receive_snapshot` で読み込む
2. スナップショット以降のイベントを `receive_recover` で再生する

## 6. no_std 環境での使い方

fraktor-rs の Persistence モジュールは `#![no_std]` で動作します。

### 依存関係

```toml
[dependencies]
fraktor-persistence-rs = { version = "0.2", default-features = false }
fraktor-actor-rs = { version = "0.2", default-features = false }
fraktor-utils-rs = { version = "0.2", default-features = false, features = ["alloc"] }
```

### 注意点

- `alloc` クレートが必要です（`extern crate alloc;`）
- ツールボックスには `NoStdToolbox` を使用します（`type TB = NoStdToolbox;`）
- TickDriver の設定が必須です。`no_std` 環境ではハードウェアパルスベースの TickDriver を構成してください
- `InMemoryJournal` / `InMemorySnapshotStore` は `no_std` で利用可能です
- 本番環境向けの Journal / SnapshotStore は `Journal` trait / `SnapshotStore` trait を実装して差し替えてください

### Journal trait の実装

カスタム Journal を実装する場合、GAT (Generic Associated Types) パターンで Future 型を定義します。

```rust
pub trait Journal: Send + Sync + 'static {
    type WriteFuture<'a>: Future<Output = Result<(), JournalError>> + Send + 'a
    where Self: 'a;

    type ReplayFuture<'a>: Future<Output = Result<Vec<PersistentRepr>, JournalError>> + Send + 'a
    where Self: 'a;

    // ...

    fn write_messages<'a>(&'a mut self, messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a>;
    fn replay_messages<'a>(&'a self, persistence_id: &'a str, from: u64, to: u64, max: u64)
        -> Self::ReplayFuture<'a>;
    fn delete_messages_to<'a>(&'a mut self, persistence_id: &'a str, to: u64)
        -> Self::DeleteFuture<'a>;
    fn highest_sequence_nr<'a>(&'a self, persistence_id: &'a str)
        -> Self::HighestSeqNrFuture<'a>;
}
```

`no_std` で `async/.await` が使えない環境でも GAT を通じて独自の Future 実装を返せます。

## 7. サンプルの実行

```bash
cargo run -p fraktor-persistence-rs --example persistent_counter_no_std
```

完全な実装は `modules/persistence/examples/persistent_counter_no_std/main.rs` を参照してください。

## 8. 関連ガイド

| ガイド | 説明 |
|--------|------|
| [Getting Started](getting-started.md) | fraktor-rs の基本的な使い方 |
| [ActorSystem ガイド](actor-system.md) | システムの初期化・停止フロー |
| [TickDriver クイックスタート](tick-driver-quickstart.md) | スケジューラのタイマー駆動設定 |
| [Shared vs Handle](shared_vs_handle.md) | 共有ラッパーの命名・分離ガイド |
