# fraktor-rs APIリファレンス

## 概要

fraktor-rsは、Akka/Pekko互換のアクターランタイムをRust/no_stdで実装したライブラリです。このドキュメントでは、主要なAPIとその使用方法について説明します。

## コアコンセプト

### アクターモデル

fraktor-rsは、メッセージパッシングによる並行処理を実現するアクターモデルを採用しています。

- **Actor**: 状態とメッセージ処理ロジックをカプセル化
- **ActorSystem**: アクターの生成と管理を担当
- **Mailbox**: アクター間のメッセージキュー
- **Supervisor**: 子アクターの障害処理を担当

## 主要API

### 1. Actor トレイト

アクターの基本的な振る舞いを定義するトレイトです。

```rust
pub trait Actor {
    /// アクター起動時に呼び出されます
    fn pre_start(
        &mut self,
        ctx: &mut ActorContext<'_>,
        stage: LifecycleStage,
    ) -> Result<(), ActorError>;

    /// メッセージ受信時に呼び出されます
    fn receive(
        &mut self,
        ctx: &mut ActorContext<'_>,
        message: &dyn Any,
    ) -> Result<(), ActorError>;

    /// アクター停止時に呼び出されます
    fn post_stop(
        &mut self,
        ctx: &mut ActorContext<'_>,
        stage: LifecycleStage,
    ) -> Result<(), ActorError>;

    /// 監視対象アクターの終了通知を受け取ります
    fn on_terminated(
        &mut self,
        ctx: &mut ActorContext<'_>,
        terminated: Pid,
    ) -> Result<(), ActorError>;
}
```

#### メソッド

##### `pre_start`

アクターが起動または再起動される時に呼び出されます。

**パラメータ**:
- `ctx`: アクターコンテキスト
- `stage`: ライフサイクルステージ（Started / Restarted）

**使用例**:
```rust
fn pre_start(
    &mut self,
    ctx: &mut ActorContext<'_>,
    stage: LifecycleStage,
) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "Actor started");
    Ok(())
}
```

##### `receive`

アクターがメッセージを受信した時に呼び出されます。

**パラメータ**:
- `ctx`: アクターコンテキスト
- `message`: 受信したメッセージ（`&dyn Any`）

**使用例**:
```rust
fn receive(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: &dyn Any,
) -> Result<(), ActorError> {
    if let Some(msg) = message.downcast_ref::<String>() {
        ctx.log(LogLevel::Info, format!("Received: {}", msg));
    }
    Ok(())
}
```

##### `post_stop`

アクターが停止する時に呼び出されます。

**パラメータ**:
- `ctx`: アクターコンテキスト
- `stage`: ライフサイクルステージ（Stopped / Restarted）

**使用例**:
```rust
fn post_stop(
    &mut self,
    ctx: &mut ActorContext<'_>,
    stage: LifecycleStage,
) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, "Actor stopped");
    Ok(())
}
```

##### `on_terminated`

監視対象のアクターが終了した時に呼び出されます（DeathWatch API）。

**パラメータ**:
- `ctx`: アクターコンテキスト
- `terminated`: 終了したアクターのPID

**使用例**:
```rust
fn on_terminated(
    &mut self,
    ctx: &mut ActorContext<'_>,
    terminated: Pid,
) -> Result<(), ActorError> {
    ctx.log(LogLevel::Info, format!("{:?} terminated", terminated));
    // 必要に応じて復旧処理を実行
    Ok(())
}
```

### 2. ActorContext

アクターの実行コンテキストを提供する構造体です。

```rust
pub struct ActorContextGeneric<'a, RT> {
    system: &'a ActorSystemGeneric<RT>,
    pid: Pid,
    reply_to: Option<Pid>,
    _marker: PhantomData<RT>,
}
```

#### 主要メソッド

##### メッセージ送信

```rust
// 通常のメッセージ送信（tell）
ctx.tell(&target_pid, message)?;

// 返信メッセージの送信（reply）
ctx.reply(response_message)?;

// 質問メッセージの送信（ask）
let future = ctx.ask(&target_pid, message)?;
```

##### アクター監視（DeathWatch）

```rust
// アクター監視の開始
ctx.watch(child.actor_ref())?;

// アクター監視の解除
ctx.unwatch(child.actor_ref())?;

// 子アクターの生成と監視を同時に実行
let child = ctx.spawn_child_watched(props)?;
```

##### 子アクター管理

```rust
// 子アクターの生成
let child = ctx.spawn_child(props)?;

// 名前付き子アクターの生成
let child = ctx.spawn_child_named(props, "child-name")?;

// アクターの停止
ctx.stop(&child_pid)?;
```

##### ロギング

```rust
ctx.log(LogLevel::Info, "Information message");
ctx.log(LogLevel::Warn, "Warning message");
ctx.log(LogLevel::Error, "Error message");
```

### 3. ActorSystem

アクターシステムの実装です。

```rust
pub struct ActorSystemGeneric<RT> {
    state: Arc<ActorSystemState<RT>>,
}
```

#### 主要メソッド

##### システムの作成

```rust
// 空のシステムを作成
let system = ActorSystemGeneric::new_empty(toolbox)?;

// Propsからシステムを作成
let system = ActorSystemGeneric::new(&props)?;
```

##### アクターの生成

```rust
// ルートアクターとして生成
let actor_ref = system.spawn_with_parent(&props, &parent_pid)?;

// 名前付きアクターの生成
let actor_ref = system.spawn_named(&props, "actor-name")?;
```

##### システムへのアクセス

```rust
// システムガーディアンの取得
let guardian = system.system_guardian_ref();

// ユーザーガーディアンの取得
let user_guardian = system.user_guardian_ref();
```

### 4. Pid (Process Identifier)

アクターの一意な識別子です。

```rust
pub struct Pid {
    value: u64,
    generation: u64,
}
```

#### 主要メソッド

```rust
// PIDの生成
let pid = Pid::new(value, generation);

// PIDの取得
let value = pid.value();
let generation = pid.generation();

// PIDの比較
if pid1 == pid2 {
    // 同じアクター
}
```

### 5. SystemMessage

システムレベルのメッセージ型です。

```rust
pub enum SystemMessage {
    /// アクターの停止
    Stop,
    /// アクターの作成
    Create { props: Props },
    /// アクターの再作成
    Recreate { props: Props },
    /// アクターの一時停止
    Suspend,
    /// アクターの再開
    Resume,
    /// アクターの監視開始
    Watch { watcher: Pid },
    /// アクターの監視解除
    Unwatch { watcher: Pid },
    /// アクターの終了通知
    Terminated { pid: Pid },
    /// 子アクターの失敗通知
    Failure { failed: Pid, error: ActorError },
}
```

#### 用途

- **Stop**: アクターの停止を指示
- **Create**: アクターのインスタンス作成
- **Recreate**: アクターの再起動
- **Suspend/Resume**: アクターの一時停止と再開
- **Watch/Unwatch**: DeathWatch APIの実装
- **Terminated**: 監視対象アクターの終了通知
- **Failure**: 子アクターの失敗を親に通知

### 6. SupervisorStrategy

子アクターの障害処理戦略を定義します。

```rust
pub struct SupervisorStrategy {
    kind: SupervisorStrategyKind,
    max_restarts: Option<usize>,
    within: Option<Duration>,
    decider: Arc<dyn SupervisionDecider + Send + Sync>,
}
```

#### 戦略の種類

##### OneForOne

失敗した子アクターのみを再起動します。

```rust
let strategy = SupervisorStrategy::one_for_one()
    .with_max_restarts(3)
    .with_within(Duration::from_secs(60))
    .build()?;
```

##### AllForOne

いずれかの子アクターが失敗した場合、すべての子アクターを再起動します。

```rust
let strategy = SupervisorStrategy::all_for_one()
    .with_max_restarts(3)
    .with_within(Duration::from_secs(60))
    .build()?;
```

#### Directive

障害時の処理を指定します。

- **Resume**: アクターを再開（状態を保持）
- **Restart**: アクターを再起動（状態をリセット）
- **Stop**: アクターを停止
- **Escalate**: 親アクターに問題をエスカレート

```rust
impl SupervisionDecider for MyDecider {
    fn decide(&self, error: &ActorError) -> Directive {
        match error {
            ActorError::Temporary => Directive::Restart,
            ActorError::Fatal => Directive::Stop,
            _ => Directive::Escalate,
        }
    }
}
```

## Typed API（実験的）

メッセージ型をコンパイル時に固定したい場合に使用できる型安全なAPIです。

### TypedActor トレイト

```rust
pub trait TypedActor<RT, M> {
    fn receive(
        &mut self,
        ctx: &mut TypedActorContextGeneric<'_, RT, M>,
        message: &M,
    ) -> Result<(), ActorError>;
}
```

### 使用例

```rust
#[derive(Clone, Copy)]
enum CounterMessage {
    Increment(i32),
    Read,
}

struct CounterActor {
    total: i32,
}

impl TypedActor<NoStdToolbox, CounterMessage> for CounterActor {
    fn receive(
        &mut self,
        ctx: &mut TypedActorContextGeneric<'_, NoStdToolbox, CounterMessage>,
        message: &CounterMessage,
    ) -> Result<(), ActorError> {
        match message {
            CounterMessage::Increment(delta) => {
                self.total += delta;
                Ok(())
            },
            CounterMessage::Read => {
                ctx.reply(self.total)
                   .map_err(|error| ActorError::from_send_error(&error))
            },
        }
    }
}

// システムの作成
let behavior = BehaviorGeneric::<NoStdToolbox, CounterMessage>::new(CounterActor::new);
let system = TypedActorSystemGeneric::new(&behavior)?;
let counter = system.user_guardian_ref();

// メッセージの送信
counter.tell(CounterMessage::Increment(1))?;
let ask = counter.ask(CounterMessage::Read)?;
```

### Typed から Untyped への変換

```rust
// Typed ActorRef を Untyped に変換
let untyped_ref = typed_ref.into_untyped();

// Typed ActorSystem を Untyped に変換
let untyped_system = typed_system.as_untyped();
```

## DeathWatch API

アクターの監視と終了通知を処理するAPIです。

### 監視の開始

```rust
// アクターの監視を開始
ctx.watch(child.actor_ref())?;
```

### 監視の解除

```rust
// アクターの監視を解除
ctx.unwatch(child.actor_ref())?;
```

### 子アクターの生成と監視

```rust
// 子アクターを生成して自動的に監視
let child = ctx.spawn_child_watched(props)?;
```

### 終了通知の処理

```rust
fn on_terminated(
    &mut self,
    ctx: &mut ActorContext<'_>,
    terminated: Pid,
) -> Result<(), ActorError> {
    // 監視対象アクターが終了した時の処理
    ctx.log(LogLevel::Info, format!("{:?} has terminated", terminated));

    // 必要に応じて復旧処理を実行
    let new_child = ctx.spawn_child_watched(props)?;

    Ok(())
}
```

### 特徴

- 既に停止したアクターを監視した場合でも、即座に`SystemMessage::Terminated`が通知される
- EventStreamを経由しないため、低遅延な挙動を実現
- 復旧ロジックをActor内に閉じ込められる

## ライフサイクル制御

### アクターの起動

- `SystemMessage::Create`をmailboxに投入
- ユーザーメッセージより必ず先に処理される
- `pre_start(LifecycleStage::Started)`が呼び出される

### アクターの再起動

- `SystemMessage::Recreate`を経由
- 処理順序: `post_stop` → インスタンス再生成 → `pre_start(LifecycleStage::Restarted)`
- 送信に失敗した場合は Stop/Escalate へフォールバック

### アクターの停止

- `SystemMessage::Stop`をmailboxに投入
- `post_stop(LifecycleStage::Stopped)`が呼び出される

### 子アクターの失敗

- `SystemMessage::Failure`として親へ配送
- 監督戦略・メトリクス・EventStreamが同じ経路を共有

## エラーハンドリング

### ActorError

アクター処理で発生するエラーを表現します。

```rust
pub enum ActorError {
    // エラー種別
    MailboxError,
    SendError,
    ActorNotFound,
    ActorAlreadyExists,
    // ...その他のエラー
}
```

### エラーからの復旧

```rust
match ctx.tell(&target, message) {
    Ok(_) => {
        // メッセージ送信成功
    },
    Err(ActorError::ActorNotFound) => {
        // アクターが見つからない場合の処理
        ctx.log(LogLevel::Warn, "Target actor not found");
    },
    Err(e) => {
        // その他のエラー処理
        return Err(e);
    }
}
```

## EventStream

システム全体のイベントを購読できる仕組みです。

### イベントの購読

```rust
// イベントリスナーの登録
system.event_stream().subscribe(listener_pid, EventType::ActorCreated)?;
```

### イベントの発行

```rust
// イベントの発行
system.event_stream().publish(Event::ActorCreated { pid })?;
```

### イベント種別

- **ActorCreated**: アクター作成
- **ActorStopped**: アクター停止
- **ActorFailed**: アクター失敗
- **Mailbox**: メールボックスイベント
- **DeadLetter**: デッドレター

## まとめ

fraktor-rsは、Akka/Pekko互換のアクターモデルをRust/no_std環境で実現するライブラリです。

### 主要な機能

- **Actor トレイト**: アクターの振る舞いを定義
- **ActorSystem**: アクターの生成と管理
- **DeathWatch API**: アクターの監視と終了通知
- **Supervisor Strategy**: 子アクターの障害処理
- **Typed API**: コンパイル時の型安全性（実験的）

### 次のステップ

- [プロジェクト構造](./project-structure.md)
- [使用例](../modules/actor-std/examples/)
- [開発ガイド](../docs/guides/)
