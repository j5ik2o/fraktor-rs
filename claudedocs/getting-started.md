# cellactor-rs 使用開始ガイド

## はじめに

cellactor-rsは、Akka/Pekko互換のアクターランタイムをRust/no_std環境で実装したライブラリです。このガイドでは、cellactor-rsを使い始めるための基本的な手順を説明します。

## インストール

### Cargo.tomlへの追加

標準環境（std）で使用する場合:

```toml
[dependencies]
cellactor-actor-core-rs = { path = "path/to/modules/actor-core" }
cellactor-actor-std-rs = { path = "path/to/modules/actor-std" }
```

no_std環境で使用する場合:

```toml
[dependencies]
cellactor-actor-core-rs = { path = "path/to/modules/actor-core", default-features = false }
cellactor-utils-core-rs = { path = "path/to/modules/utils-core", default-features = false }
```

## 基本的な使い方

### 1. 最初のアクター

最も簡単なアクターを作成してみましょう。

```rust
use cellactor_actor_core_rs::{
    Actor, ActorContext, ActorError, ActorSystem, ActorSystemGeneric,
    LifecycleStage, LogLevel, Props,
};
use cellactor_actor_std_rs::{StdActorSystem, StdToolbox};
use core::any::Any;

// アクターの定義
struct HelloActor {
    name: String,
}

impl HelloActor {
    fn new(name: String) -> Self {
        Self { name }
    }
}

// Actorトレイトの実装
impl Actor for HelloActor {
    fn pre_start(
        &mut self,
        ctx: &mut ActorContext<'_>,
        _stage: LifecycleStage,
    ) -> Result<(), ActorError> {
        ctx.log(LogLevel::Info, format!("Hello from {}!", self.name));
        Ok(())
    }

    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        if let Some(msg) = message.downcast_ref::<String>() {
            ctx.log(LogLevel::Info, format!("{} received: {}", self.name, msg));
        }
        Ok(())
    }

    fn post_stop(
        &mut self,
        ctx: &mut ActorContext<'_>,
        _stage: LifecycleStage,
    ) -> Result<(), ActorError> {
        ctx.log(LogLevel::Info, format!("Goodbye from {}!", self.name));
        Ok(())
    }
}

fn main() -> Result<(), ActorError> {
    // Propsの作成
    let props = Props::from_fn(|| HelloActor::new("Alice".to_string()));

    // ActorSystemの作成
    let system = StdActorSystem::new(&props)?;

    // メッセージの送信
    let actor_ref = system.user_guardian_ref();
    actor_ref.tell("Hello, World!".to_string())?;

    // システムの停止
    system.terminate()?;
    system.when_terminated().wait();

    Ok(())
}
```

### 2. メッセージングパターン

#### Tell（一方向メッセージ）

最も基本的なメッセージ送信パターンです。

```rust
// メッセージの送信
actor_ref.tell("Hello!".to_string())?;
```

#### Ask（リクエスト・レスポンスパターン）

返信が必要な場合は、askパターンを使用します。

```rust
// カスタムメッセージ型
#[derive(Clone)]
struct Query {
    question: String,
    reply_to: ActorRef,
}

#[derive(Clone)]
struct Response {
    answer: String,
}

// アクターでの処理
impl Actor for QueryActor {
    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        if let Some(Query { question, reply_to }) = message.downcast_ref::<Query>() {
            // 質問を処理
            let answer = self.process_query(question);

            // 返信
            reply_to.tell(Response { answer })?;
        }
        Ok(())
    }
}

// 送信側
let query = Query {
    question: "What is the meaning of life?".to_string(),
    reply_to: ctx.self_ref(),
};
target_actor.tell(query)?;
```

### 3. 子アクターの管理

親アクターは子アクターを生成し、管理することができます。

```rust
struct ParentActor {
    children: Vec<Pid>,
}

impl Actor for ParentActor {
    fn pre_start(
        &mut self,
        ctx: &mut ActorContext<'_>,
        _stage: LifecycleStage,
    ) -> Result<(), ActorError> {
        // 子アクターの生成
        let child_props = Props::from_fn(|| ChildActor::new());
        let child = ctx.spawn_child(child_props)?;

        self.children.push(child.pid());

        ctx.log(LogLevel::Info, "Child actor created");
        Ok(())
    }

    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        // 子アクターにメッセージを転送
        for child_pid in &self.children {
            let child_ref = ctx.system().actor_ref(child_pid)?;
            child_ref.tell(message)?;
        }
        Ok(())
    }
}
```

### 4. DeathWatch API

アクターの終了を監視できます。

```rust
struct WatcherActor {
    watched_pid: Option<Pid>,
}

impl Actor for WatcherActor {
    fn pre_start(
        &mut self,
        ctx: &mut ActorContext<'_>,
        _stage: LifecycleStage,
    ) -> Result<(), ActorError> {
        // 子アクターを生成して監視
        let child_props = Props::from_fn(|| ChildActor::new());
        let child = ctx.spawn_child_watched(child_props)?;

        self.watched_pid = Some(child.pid());

        ctx.log(LogLevel::Info, "Watching child actor");
        Ok(())
    }

    fn on_terminated(
        &mut self,
        ctx: &mut ActorContext<'_>,
        terminated: Pid,
    ) -> Result<(), ActorError> {
        if Some(terminated) == self.watched_pid {
            ctx.log(LogLevel::Warn, "Child actor terminated, restarting...");

            // 新しい子アクターを生成
            let child_props = Props::from_fn(|| ChildActor::new());
            let new_child = ctx.spawn_child_watched(child_props)?;

            self.watched_pid = Some(new_child.pid());
        }
        Ok(())
    }
}
```

### 5. 監督戦略（Supervisor Strategy）

子アクターの障害を処理する戦略を設定できます。

```rust
use cellactor_actor_core_rs::{
    SupervisorStrategy, SupervisorStrategyKind, Directive, SupervisionDecider,
};
use core::time::Duration;

// カスタムのDeciderを実装
struct MyDecider;

impl SupervisionDecider for MyDecider {
    fn decide(&self, error: &ActorError) -> Directive {
        match error {
            ActorError::Temporary => Directive::Restart,
            ActorError::Fatal => Directive::Stop,
            _ => Directive::Escalate,
        }
    }
}

// OneForOne戦略の設定
let strategy = SupervisorStrategy::one_for_one()
    .with_max_restarts(3)
    .with_within(Duration::from_secs(60))
    .with_decider(Arc::new(MyDecider))
    .build()?;

let props = Props::from_fn(|| ChildActor::new())
    .with_supervisor_strategy(strategy);
```

### 6. Typed API（実験的）

型安全なメッセージングが必要な場合は、Typed APIを使用できます。

```rust
use cellactor_actor_core_rs::typed::{
    BehaviorGeneric, TypedActor, TypedActorContextGeneric,
    TypedActorSystemGeneric,
};

// メッセージ型の定義
#[derive(Clone, Copy)]
enum CounterMessage {
    Increment(i32),
    Read,
}

// Typed Actorの定義
struct CounterActor {
    total: i32,
}

impl TypedActor<StdToolbox, CounterMessage> for CounterActor {
    fn receive(
        &mut self,
        ctx: &mut TypedActorContextGeneric<'_, StdToolbox, CounterMessage>,
        message: &CounterMessage,
    ) -> Result<(), ActorError> {
        match message {
            CounterMessage::Increment(delta) => {
                self.total += delta;
                ctx.log(LogLevel::Info, format!("Total: {}", self.total));
                Ok(())
            },
            CounterMessage::Read => {
                ctx.reply(self.total)
                   .map_err(|error| ActorError::from_send_error(&error))
            },
        }
    }
}

// システムの作成と使用
fn main() -> Result<(), ActorError> {
    let behavior = BehaviorGeneric::new(|| CounterActor { total: 0 });
    let system = TypedActorSystemGeneric::new(&behavior)?;
    let counter = system.user_guardian_ref();

    // 型安全なメッセージ送信
    counter.tell(CounterMessage::Increment(5))?;
    counter.tell(CounterMessage::Increment(3))?;

    let response = counter.ask(CounterMessage::Read)?;
    // responseを待機して結果を取得

    system.terminate()?;
    system.when_terminated().wait();

    Ok(())
}
```

## 実用例

### Ping-Pongサンプル

2つのアクターが交互にメッセージを交換する例です。

```rust
// Ping アクター
struct PingActor {
    count: usize,
    max_count: usize,
}

impl Actor for PingActor {
    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        if let Some(StartPing { target, reply_to, count }) =
            message.downcast_ref::<StartPing>() {
            self.max_count = *count;
            target.tell(Ping { reply_to: reply_to.clone() })?;
        } else if let Some(Pong { .. }) = message.downcast_ref::<Pong>() {
            self.count += 1;
            if self.count < self.max_count {
                ctx.log(LogLevel::Info, format!("Ping count: {}", self.count));
            } else {
                ctx.stop_self()?;
            }
        }
        Ok(())
    }
}

// Pong アクター
struct PongActor;

impl Actor for PongActor {
    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        if let Some(Ping { reply_to }) = message.downcast_ref::<Ping>() {
            ctx.log(LogLevel::Info, "Pong");
            reply_to.tell(Pong { reply_to: ctx.self_ref() })?;
        }
        Ok(())
    }
}
```

完全なサンプルコードは、`modules/actor-std/examples/ping_pong_tokio_std/`や`modules/actor-core/examples/ping_pong_not_std/`を参照してください。

## 利用可能なサンプル

cellactor-rsには、様々な使用例が含まれています。

### std環境のサンプル

- **ping_pong_tokio_std**: Tokioランタイムとの統合例
- **death_watch_std**: DeathWatch APIの使用例
- **logger_subscriber_std**: ログ購読の例
- **named_actor_std**: 名前付きアクターの例
- **supervision_std**: 監督戦略の例
- **dead_letter_std**: デッドレター処理の例
- **behaviors_setup_receive_std**: Behaviorパターンの例
- **behaviors_receive_signal_std**: シグナル処理の例
- **behaviors_counter_typed_std**: Typed APIの例

### no_std環境のサンプル

- **ping_pong_not_std**: no_std環境での基本的な例
- **ping_pong_typed_not_std**: no_std環境でのTyped APIの例
- **death_watch_no_std**: no_std環境でのDeathWatch例
- **behaviors_setup_receive_no_std**: no_std環境でのBehaviorパターン
- **behaviors_receive_signal_no_std**: no_std環境でのシグナル処理
- **behaviors_counter_typed_no_std**: no_std環境でのTyped API

## Tokioとの統合

標準環境でTokioランタイムと統合する場合:

```rust
use cellactor_actor_std_rs::{StdActorSystem, TokioExecutor};
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() -> Result<(), ActorError> {
    let rt = Runtime::new()?;
    let handle = rt.handle().clone();

    // Tokioエグゼキュータの作成
    let executor = Arc::new(TokioExecutor::new(handle));

    // ActorSystemの作成
    let props = Props::from_fn(|| MyActor::new())
        .with_executor(executor);
    let system = StdActorSystem::new(&props)?;

    // アクターの実行
    // ...

    system.terminate()?;
    system.when_terminated().await;

    Ok(())
}
```

## EventStreamの使用

システム全体のイベントを購読できます。

```rust
use cellactor_actor_core_rs::{EventStreamEvent, LogLevel};

struct LoggerSubscriber {
    min_level: LogLevel,
}

impl Actor for LoggerSubscriber {
    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError> {
        if let Some(event) = message.downcast_ref::<EventStreamEvent>() {
            match event {
                EventStreamEvent::Log(log_event) => {
                    if log_event.level >= self.min_level {
                        println!("[{}] {}", log_event.level, log_event.message);
                    }
                },
                EventStreamEvent::ActorCreated(pid) => {
                    println!("Actor created: {:?}", pid);
                },
                EventStreamEvent::ActorStopped(pid) => {
                    println!("Actor stopped: {:?}", pid);
                },
                _ => {}
            }
        }
        Ok(())
    }
}

// EventStreamへの購読
let logger_props = Props::from_fn(|| LoggerSubscriber {
    min_level: LogLevel::Info
});
let logger = system.spawn(&logger_props)?;
system.event_stream().subscribe(logger.pid(), EventType::All)?;
```

## トラブルシューティング

### Mailboxが溢れる

capacity を増やすか、`SendError::Full`を受け取ったときにバックオフ処理を挟みます。

```rust
let props = Props::from_fn(|| MyActor::new())
    .with_mailbox_strategy(MailboxStrategy::bounded(MailboxCapacity::new(128)));
```

### 返信が届かない

payload に `reply_to` が含まれているか、メッセージ型を `downcast_ref::<T>()` で正しく解釈しているかを確認します。

```rust
#[derive(Clone)]
struct Request {
    data: String,
    reply_to: ActorRef,
}

// 送信側
let request = Request {
    data: "Hello".to_string(),
    reply_to: ctx.self_ref(),
};
target.tell(request)?;

// 受信側
if let Some(Request { data, reply_to }) = message.downcast_ref::<Request>() {
    reply_to.tell(Response::new(process(data)))?;
}
```

### Tokio連携で停止しない

`system.when_terminated()` を `await` し忘れているか、ガーディアンが自己停止していない可能性があります。

```rust
system.terminate()?;
system.when_terminated().await; // これを忘れずに
```

## 次のステップ

- [APIリファレンス](./api-reference.md): 詳細なAPI仕様
- [プロジェクト構造](./project-structure.md): プロジェクトの構成
- [ActorSystemガイド](../docs/guides/actor-system.md): ActorSystemの詳細
- [DeathWatch移行ガイド](../docs/guides/death_watch_migration.md): DeathWatch APIの移行方法
- [サンプルコード](../modules/actor-std/examples/): 実用的なサンプル集

## まとめ

cellactor-rsは、no_std環境でも動作する柔軟なアクターランタイムです。基本的なアクターの作成から、監督戦略、DeathWatch API、Typed APIまで、多様な機能を提供しています。

ご質問やフィードバックがありましたら、GitHubのissueでお知らせください。
