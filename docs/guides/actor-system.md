# ActorSystem ガイド

セルアクターランタイムの `ActorSystem` を利用する際の基本手順と、`reply_to` パターンや監視機能の運用ポイントをまとめます。no_std 環境と標準環境（Tokio 連携）で共通する設計指針を把握し、アプリケーションから安全に制御できるようにすることが目的です。

```rust
use fraktor_actor_core_rs::{ActorSystem, ActorSystemGeneric, Props};
use fraktor_actor_std_rs::{StdActorSystem, StdToolbox};
```

## 1. 初期化フロー

- **ユーザガーディアンの定義**: `Props::from_fn(|| GuardianActor)` のようにガーディアンを構築し、no_std 環境では `ActorSystem::new(&guardian_props)`、標準環境では `StdActorSystem::new(&guardian_props)` に渡します。ガーディアンはアプリケーションのエントリポイントであり、`spawn_child` を通じて子アクターを組み立てます。
- **起動メッセージ**: `system.user_guardian_ref().tell(AnyMessage::new(Start))?;` でアプリケーションを起動します。トップレベルのアクター生成はガーディアン（またはその子）経由に限定されます。
- **Mailbox / Dispatcher 構成**: `Props::with_mailbox_strategy` や `Props::with_throughput` を利用して、容量・背圧・スループットの設定を事前に行います。Bounded 戦略では容量 64 以上を推奨し、容量超過ポリシー（DropOldest など）を選択します。

```rust
let guardian_props: Props<StdToolbox> = Props::from_fn(|| GuardianActor)
  .with_mailbox_strategy(MailboxStrategy::bounded(MailboxCapacity::new(64)))
  .with_throughput(300);
let system = StdActorSystem::new(&guardian_props)?;
```

## 2. メッセージ送信と `reply_to` パターン

- ランタイムは Classic の `sender()` を提供しないため、返信が必要な場合は payload に `reply_to: ActorRef` を含めます。
- 送信側は `ctx.self_ref()` などを渡し、受信側が `reply_to.tell(...)` で応答します。
- `ask` を利用する場合は `ActorFuture` を介して待機できます。Guardian など制御側で `system.drain_ready_ask_futures()` を定期的に呼び、完了した Future を回収します。

```rust
struct StartPing {
  target:   ActorRef,
  reply_to: ActorRef,
  count:    usize,
}

ping.tell(AnyMessage::new(StartPing { target: pong, reply_to: ctx.self_ref(), count: 3 }))?;
```

## 3. 監督機能と停止フロー

- アクターは `pre_start` → `receive` → `post_stop` のライフサイクルを持ち、`ActorError::Recoverable` で再起動、`ActorError::Fatal` で停止します。
- `ctx.stop_self()` や `system.terminate()` を呼ぶと、ユーザガーディアンに `SystemMessage::Stop` が送られ、子アクターへ停止が伝播します。
- ランタイム終了待機には `system.when_terminated()` を利用し、同期環境では `run_until_terminated()`、非同期環境では `await` で待機します。

```rust
let termination = system.when_terminated();
system.terminate()?;
while !termination.is_ready() {
  core::hint::spin_loop();
}
```

## 4. 監視とオブザーバビリティ

- **EventStream**: ライフサイクル・ログ・Deadletter を publish するバスです。`system.subscribe_event_stream(subscriber)` で購読し、`on_event` で各種イベントを処理します。既定バッファ容量は 256 件で、超過すると最古のイベントから破棄されます。
- **Deadletter**: 未配達メッセージを 512 件保持し、登録時に `EventStreamEvent::Deadletter` と `LogEvent` を発火します。容量変更が必要な場合は今後追加予定の `actor-std` ヘルパー（ActorSystemConfig 仮称）での設定を検討します。
- **LoggerSubscriber**: `LogLevel` フィルタ付きで EventStream を購読し、UART/RTT やホストログへ転送します。Deadletter が 75% に達したなどの警告閾値を購読者側で判断し、任意の通知手段へ連携してください。

```rust
let logger = ArcShared::new(LoggerSubscriber::new(LogLevel::Info, ArcShared::new(MyWriter)));
let _subscription = system.subscribe_event_stream(logger);
```

## 5. Tokio ランタイムとの連携

- `modules/actor-core/examples/ping_pong_tokio` では、Tokio マルチスレッドランタイム上で Dispatcher を駆動するサンプルを確認できます。
- `DispatcherConfig::<StdToolbox>::from_executor(ArcShared::new(TokioExecutor::new(handle)))` を利用し、`Handle::spawn_blocking` 上で `dispatcher.drive()` を実行します。これにより `async fn` へ依存せずランタイム外部のスレッドプールでメッセージ処理を行えます。
- 今後 `actor-std` クレートへ追加する拡張 API（例: Tokio ランタイムハンドルからの安全な取得）については、本ガイドと quickstart を同時に更新し、no_std な `actor-core` への追加依存が発生しないようにします。

## 6. トラブルシュートのヒント

- **Mailbox が溢れる**: capacity を増やすか、`SendError::Full` を受け取ったときにバックオフ処理を挟みます。Deadletter の Warn ログを監視し、容量やポリシーを再調整してください。
- **返信が届かない**: payload に `reply_to` が含まれているか、メッセージ型を `downcast_ref::<T>()` で正しく解釈しているかを確認します。
- **Tokio 連携で停止しない**: `system.when_terminated()` を `await` し忘れているか、ガーディアンが自己停止していない可能性があります。`system.terminate()` 後に `when_terminated()` の完了を待機してください。

以上の手順と注意点を押さえておけば、組込みからホスト環境まで一貫した ActorSystem の運用と監視が可能になります。今後 `actor-std` にヘルパー API が追加された際は、本ガイドと quickstart を更新し、想定されるブートストラップ手順を最新化してください。

## 7. typed vs untyped アクターの選択ガイド

fraktor-rs は **typed actor**（型安全なアクター）と **untyped actor**（動的ディスパッチのアクター）の2つのスタイルを提供しています。ここでは、どちらを選ぶべきかの判断フローと、それぞれの使い方を説明します。

### 判断フロー

```
メッセージ型が1つの enum / struct に決まるか？
├─ Yes → typed actor（推奨）
│   ├─ 状態を struct で保持し、ライフサイクルフックが必要？
│   │   └─ Yes → TypedActor<M> trait を実装
│   └─ 関数型スタイルで状態遷移を表現したい？
│       └─ Yes → Behavior DSL（Behaviors::receive_message 等）
├─ ほぼ決まるが、一部外部プロトコルとの橋渡しが必要
│   └─ typed actor + Message Adapter（7.3 参照）
└─ No（複数の無関係な型を受け取る、型統一が不可能）
    └─ untyped actor（Actor trait + AnyMessageView で downcast）
```

### 7.1 推奨: typed actor

typed actor はメッセージ型 `M` をジェネリクスで固定し、コンパイル時に型安全性を保証します。fraktor-rs では原則としてこちらを推奨します。

**利点**:
- `TypedActorRef<M>` 経由の `tell(M)` がコンパイル時に型チェックされる
- `downcast_ref` の実行時エラーが発生しない
- IDE の補完・リファクタリング支援が効果的に働く
- メッセージプロトコルがシグネチャに明示される

typed actor を定義するには2つの方法があります。

#### 方法 A: `TypedActor<M>` trait の実装

状態をフィールドに持ち、`pre_start` / `post_stop` / `on_terminated` 等のライフサイクルフックを活用する場合に適しています。no_std 環境でも利用可能です。

```rust
use fraktor_actor_rs::core::{
  error::ActorError,
  typed::{
    TypedActorSystem, TypedProps,
    actor::{TypedActor, TypedActorContext, TypedActorRef},
  },
};

// メッセージ型を enum で定義
enum CounterCommand {
  Add(i32),
  Read { reply_to: TypedActorRef<i32> },
}

// アクター構造体
struct CounterActor {
  total: i32,
}

impl TypedActor<CounterCommand> for CounterActor {
  fn receive(
    &mut self,
    _ctx: &mut TypedActorContext<'_, CounterCommand>,
    message: &CounterCommand,
  ) -> Result<(), ActorError> {
    match message {
      CounterCommand::Add(delta) => {
        self.total += delta;
      },
      CounterCommand::Read { reply_to } => {
        reply_to.clone().tell(self.total)
          .map_err(|error| ActorError::from_send_error(&error))?;
      },
    }
    Ok(())
  }
}

// 起動
let props = TypedProps::new(|| CounterActor { total: 0 });
let system = TypedActorSystem::new(&props, tick_driver).expect("system");
let mut counter = system.user_guardian_ref();
counter.tell(CounterCommand::Add(10)).expect("tell");
```

#### 方法 B: Behavior DSL

Apache Pekko に着想を得た関数型スタイルです。状態遷移を「新しい `Behavior` を返す」ことで表現します。no_std / std 両方で利用可能です。

```rust
use fraktor_actor_rs::core::typed::{Behavior, Behaviors, TypedActorSystem, TypedProps};

enum CounterCommand {
  Add(i32),
}

// 状態を関数の引数としてキャプチャし、新しい Behavior を返す
fn counter(total: i32) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    CounterCommand::Add(delta) => Ok(counter(total + delta)),
  })
}

let props = TypedProps::from_behavior_factory(|| counter(0));
let system = TypedActorSystem::new(&props, tick_driver).expect("system");
```

**Behavior DSL の主要 API**:

| API | 用途 |
|-----|------|
| `Behaviors::receive_message(handler)` | メッセージハンドラを定義し `Behavior` を構築する |
| `Behaviors::receive_and_reply(handler)` | 現在の sender に返信し、`Behavior::same()` を返す |
| `Behaviors::setup(\|ctx\| behavior)` | 起動時にコンテキストを利用して初期化する |
| `Behaviors::receive_signal(handler)` | `BehaviorSignal`（Started, Stopped, Terminated 等）を処理する |
| `Behaviors::supervise(behavior)` | 子アクターの監督戦略を宣言的に設定する |
| `Behaviors::same()` | 現在の Behavior を維持する |
| `Behaviors::stopped()` | アクターを停止する |
| `Behaviors::ignore()` | メッセージを無視する |
| `Behaviors::unhandled()` | 未処理メッセージとしてイベントストリームに通知する |
| `Behaviors::empty()` | すべてのメッセージを未処理として扱う（停止待機中等） |

**状態遷移の例**: 改札ゲート（locked / unlocked の2状態を遷移）

```rust
fn locked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    GateCommand::InsertCoin => Ok(unlocked(pass_count)),
    GateCommand::PassThrough => Ok(Behaviors::ignore()),
    GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}

fn unlocked(pass_count: u32) -> Behavior<GateCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    GateCommand::PassThrough => Ok(locked(pass_count + 1)),
    GateCommand::InsertCoin => Ok(Behaviors::ignore()),
    GateCommand::Shutdown => Ok(Behaviors::stopped()),
  })
}
```

**`Behaviors::setup` + `receive_message` の組み合わせ**: 起動時に子アクターを生成し、以降のメッセージ処理で利用する

```rust
fn guardian_behavior() -> Behavior<GuardianCommand> {
  Behaviors::setup(|ctx| {
    let worker = ctx.spawn_child(&worker_props).expect("spawn").actor_ref();
    Behaviors::receive_message(move |_ctx, message| match message {
      GuardianCommand::Start => {
        worker.clone().tell(WorkerCommand { text: "hello" }).expect("tell");
        Ok(Behaviors::same())
      },
    })
  })
}
```

**監督戦略の宣言**: `Behaviors::supervise` で子アクター障害時のポリシーを設定する

```rust
let behavior = Behaviors::setup(|ctx| {
  ctx.spawn_child(&child_props).expect("spawn");
  Behaviors::receive_message(|_ctx, _msg| Ok(Behaviors::same()))
});
let strategy = SupervisorStrategy::new(
  SupervisorStrategyKind::OneForOne, 5, Duration::from_secs(1),
  |error| match error {
    ActorError::Recoverable(_) => SupervisorDirective::Restart,
    ActorError::Fatal(_) => SupervisorDirective::Stop,
  },
);
Behaviors::supervise(behavior).on_failure(strategy)
```

**`receive_and_reply` の例**: ask パターンの定型を簡潔に記述する

```rust
enum CounterQuery {
  GetTotal,
}

fn counter(total: i32) -> Behavior<CounterQuery> {
  Behaviors::receive_and_reply(move |_ctx, message| match message {
    CounterQuery::GetTotal => Ok(total),
  })
}
```

#### TypedActor と Behavior DSL の使い分け

| 観点 | TypedActor<M> trait | Behavior DSL |
|------|---------------------|--------------|
| 状態管理 | struct フィールドとして保持 | 関数引数のクロージャキャプチャ |
| ライフサイクルフック | `pre_start` / `post_stop` / `on_terminated` を個別にオーバーライド | `receive_signal` で `BehaviorSignal` を処理 |
| 状態遷移 | フィールドの更新（`&mut self`） | 新しい `Behavior` を返す（関数型） |
| 監督戦略 | `supervisor_strategy` メソッドで動的に決定 | `Behaviors::supervise().on_failure()` で宣言的に設定 |
| Message Adapter | `pre_start` 等で `ctx.message_adapter()` を利用 | `setup` 内で `ctx.message_adapter()` を利用 |
| 適用場面 | ライフサイクル管理が複雑、手続き的なスタイルを好む | 状態遷移が明確、関数型スタイルを好む |

### 7.2 代替: untyped actor

メッセージ型を1つに絞れない場合（動的ディスパッチ、heterogeneous なアクターツリー、プラグインシステム等）に使用します。

```rust
use fraktor_actor_rs::core::{
  actor::{Actor, ActorContext, actor_ref::ActorRef},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::ActorSystem,
  error::ActorError,
};

struct Start;
struct Greeting { text: String }

struct MyActor;

impl Actor for MyActor {
  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: AnyMessageViewGeneric<'_>,
  ) -> Result<(), ActorError> {
    // 実行時に downcast して型を判定する
    if message.downcast_ref::<Start>().is_some() {
      // 起動処理
    } else if let Some(greeting) = message.downcast_ref::<Greeting>() {
      // 挨拶処理
    }
    Ok(())
  }
}

let props = Props::from_fn(|| MyActor);
let system = ActorSystem::new(&props, tick_driver).expect("system");
system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");
```

**untyped actor の特徴**:

| 特徴 | 説明 |
|------|------|
| メッセージ型 | `AnyMessage` / `AnyMessageViewGeneric` — 任意の `Send + Sync + 'static` 型を送受信可能 |
| 型チェック | 実行時に `downcast_ref::<T>()` で判定（失敗時は暗黙的に無視される） |
| アクター参照 | `ActorRef`（untyped）— `tell(AnyMessage::new(msg))` で送信 |
| ユースケース | 複数の無関係なメッセージ型を受け取る、プラグインシステム、レガシーコードとの統合 |

### 7.3 Message Adapter: typed actor 間のプロトコル変換

異なるメッセージ型を持つ typed actor 同士を接続する場合、untyped に落とさなくても **Message Adapter** を使えば型安全に変換できます。

**TypedActor での利用例**:

```rust
impl TypedActor<CounterCommand> for CounterActor {
  fn pre_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, CounterCommand>,
  ) -> Result<(), ActorError> {
    // String → CounterCommand への変換アダプタを登録
    let adapter = ctx.message_adapter(|payload: String| {
      payload.parse::<i32>()
        .map(CounterCommand::Apply)
        .map_err(|_| AdapterError::Custom("parse error".into()))
    }).map_err(|e| ActorError::Recoverable(e.to_string().into()))?;
    // adapter は TypedActorRef<String> — String を受け取れる参照
    // これを他のアクターに渡せば、String で送信してもらえる
    self.notify.tell(GuardianEvent::AdapterReady(adapter))
      .map_err(|e| ActorError::from_send_error(&e))?;
    Ok(())
  }
}
```

**Behavior DSL での利用例**:

```rust
fn counter(total: i32) -> Behavior<CounterCommand> {
  Behaviors::setup(|ctx| {
    // setup 内でもアダプタ登録が可能
    let _adapter = ctx.message_adapter(|payload: String| {
      payload.parse::<i32>()
        .map(CounterCommand::Apply)
        .map_err(|_| AdapterError::Custom("parse error".into()))
    });
    Behaviors::receive_message(move |_ctx, message| match message {
      CounterCommand::Apply(delta) => Ok(counter(total + delta)),
    })
  })
}
```

Message Adapter は TypedActor / Behavior DSL の両方で利用でき、typed actor のメッセージ型を変更せずに外部プロトコルとの橋渡しが可能です。アダプタの変換失敗時は `BehaviorSignal::AdapterFailed` としてシグナルハンドラに通知されます。
