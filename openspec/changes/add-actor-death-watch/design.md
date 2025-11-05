# 設計書: ActorContext watch/unwatch API - DeathWatch実装

## アーキテクチャ概要

### 設計原則

**単一イベント型による表現（Option 1方式）**:
- `LifecycleEvent`に`watcher: Option<Pid>`フィールドを追加
- 同じ事象（アクター終了）を単一のイベント型で表現（DRY原則）
- `watcher: None` → システムワイド観測可能性イベント
- `watcher: Some(pid)` → 特定監視者向けDeathWatchイベント

### なぜ単一イベント型か

#### 従来案（Option 2）の問題点
```rust
// ❌ 従来案: 2つの独立したイベント型
pub enum EventStreamEvent {
  Lifecycle(LifecycleEvent),      // システムワイド観測
  Terminated(TerminatedEvent),    // DeathWatch専用
  // ...
}
```

**問題**:
- 同じアクター停止という事象を2つのイベント型で表現（重複）
- EventStreamEventに新しいvariantを追加する必要がある
- イベント処理ロジックが2箇所に分散

#### 採用案（Option 1）の利点
```rust
// ✅ 採用案: 単一イベント型の拡張
pub struct LifecycleEvent {
  pid: Pid,
  parent: Option<Pid>,
  name: String,
  stage: LifecycleStage,
  timestamp: Duration,
  watcher: Option<Pid>,  // NEW
}
```

**利点**:
1. **概念的一貫性**: 1つの事象 = 1つのイベント型
2. **実装の簡潔さ**: EventStreamEventに手を加えない
3. **拡張性**: Started/Restartedステージでも将来的にwatcherベースイベントに対応可能
4. **DRY原則**: "同じことを2つの方法で表現しない"

## コンポーネント設計

### 1. LifecycleEvent拡張（破壊的変更）

**ファイル**: `modules/actor-core/src/lifecycle/lifecycle_event.rs`

```rust
/// ライフサイクルイベント - アクターの状態遷移を表現
#[derive(Clone, Debug)]
pub struct LifecycleEvent {
  pid: Pid,
  parent: Option<Pid>,
  name: String,
  stage: LifecycleStage,
  timestamp: Duration,

  /// 監視者のPid
  /// - None: システムワイド観測イベント（全EventStreamサブスクライバー向け）
  /// - Some(pid): 特定の監視者向けDeathWatchイベント
  watcher: Option<Pid>,
}

impl LifecycleEvent {
  /// システムワイドStartedイベント生成
  pub fn new_started(
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    timestamp: Duration,
  ) -> Self {
    Self {
      pid,
      parent,
      name,
      stage: LifecycleStage::Started,
      timestamp,
      watcher: None,
    }
  }

  /// システムワイドRestartedイベント生成
  pub fn new_restarted(
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    timestamp: Duration,
  ) -> Self {
    Self {
      pid,
      parent,
      name,
      stage: LifecycleStage::Restarted,
      timestamp,
      watcher: None,
    }
  }

  /// システムワイドStoppedイベント生成
  pub fn new_stopped(
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    timestamp: Duration,
  ) -> Self {
    Self {
      pid,
      parent,
      name,
      stage: LifecycleStage::Stopped,
      timestamp,
      watcher: None,
    }
  }

  /// 監視者向け終了通知イベント生成（DeathWatch用）
  pub fn new_terminated(
    pid: Pid,
    parent: Option<Pid>,
    name: String,
    timestamp: Duration,
    watcher: Pid,
  ) -> Self {
    Self {
      pid,
      parent,
      name,
      stage: LifecycleStage::Stopped,
      timestamp,
      watcher: Some(watcher),
    }
  }

  /// このイベントが監視者向けかどうかを判定
  pub const fn is_watched(&self) -> bool {
    self.watcher.is_some()
  }

  /// 監視者のPidを取得
  pub const fn watcher(&self) -> Option<Pid> {
    self.watcher
  }

  // 既存のメソッド
  pub const fn pid(&self) -> Pid { self.pid }
  pub const fn parent(&self) -> Option<Pid> { self.parent }
  pub fn name(&self) -> &str { &self.name }
  pub const fn stage(&self) -> LifecycleStage { self.stage }
  pub const fn timestamp(&self) -> Duration { self.timestamp }
}
```

**破壊的変更の緩和策**:
1. 便利メソッド（`new_started`等）を提供し、既存コードは最小限の変更で済む
2. 構造体フィールドは`pub`のまま維持し、直接生成も可能
3. パターンマッチングには`..`を使えば新フィールドを無視できる

### 2. SystemMessage拡張

**ファイル**: `modules/actor-core/src/messaging/system_message.rs`

```rust
/// システムメッセージ
#[derive(Clone, Debug)]
pub enum SystemMessage {
  Stop,
  Suspend,
  Resume,

  /// 指定したアクター（監視者）がこのアクターを監視開始
  Watch(Pid),

  /// 指定したアクター（監視者）がこのアクターの監視を解除
  Unwatch(Pid),
}
```

### 3. ActorCell拡張

**ファイル**: `modules/actor-core/src/actor_prim/actor_cell.rs`

```rust
pub struct ActorCellGeneric<TB: RuntimeToolbox, A: Actor<TB>> {
  // 既存フィールド
  pid: Pid,
  parent: Option<Pid>,
  actor: ToolboxMutex<A, TB>,
  sender: ArcShared<dyn ActorRefSender<TB>>,
  children: ToolboxMutex<Vec<Pid>, TB>,
  child_stats: ToolboxMutex<Vec<RestartStatistics>>,
  supervisor: Option<SupervisorStrategy>,
  mailbox: ArcShared<dyn Mailbox<TB>>,
  system: ArcShared<SystemStateGeneric<TB>>,

  /// NEW: このアクターを監視している監視者のリスト
  watchers: ToolboxMutex<Vec<Pid>, TB>,
}

impl<TB: RuntimeToolbox, A: Actor<TB>> ActorCellGeneric<TB, A> {
  /// ActorCellを作成
  pub(crate) fn new(
    pid: Pid,
    parent: Option<Pid>,
    factory: Box<dyn ActorFactory<TB, Actor = A>>,
    props: &PropsGeneric<TB>,
    sender: ArcShared<dyn ActorRefSender<TB>>,
    system: ArcShared<SystemStateGeneric<TB>>,
  ) -> Result<Self, ActorError> {
    // 既存の初期化コード...

    Ok(Self {
      pid,
      parent,
      actor,
      sender,
      children: ToolboxMutex::new(Vec::new()),
      child_stats: ToolboxMutex::new(Vec::new()),
      supervisor,
      mailbox,
      system,
      watchers: ToolboxMutex::new(Vec::new()), // NEW
    })
  }

  /// 監視者を追加（SystemMessage::Watch処理）
  pub(crate) fn handle_watch(&self, watcher: Pid) {
    let mut watchers = self.watchers.lock();

    // 冪等性: 既に監視者リストにいる場合は追加しない
    if !watchers.contains(&watcher) {
      watchers.push(watcher);
    }
  }

  /// 監視者を削除（SystemMessage::Unwatch処理）
  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.watchers.lock().retain(|pid| *pid != watcher);
  }

  /// 停止時に監視者に通知
  fn notify_watchers_on_stop(&self) {
    let watchers = self.watchers.lock().clone();
    let timestamp = self.system.monotonic_now();

    // 各監視者向けにLifecycleEvent(watcher: Some(pid))を発行
    for watcher_pid in watchers {
      let event = LifecycleEvent::new_terminated(
        self.pid,
        self.parent,
        self.pid.name().to_string(),
        timestamp,
        watcher_pid,
      );

      self.system.publish_event(EventStreamEvent::Lifecycle(event));
    }

    // システムワイド観測用のLifecycleEvent(watcher: None)も発行
    let system_event = LifecycleEvent::new_stopped(
      self.pid,
      self.parent,
      self.pid.name().to_string(),
      timestamp,
    );

    self.system.publish_event(EventStreamEvent::Lifecycle(system_event));
  }

  /// アクター停止処理（既存メソッドに統合）
  pub(crate) fn stop(&self) -> Result<(), ActorError> {
    // 既存の停止処理...

    // NEW: 監視者への通知
    self.notify_watchers_on_stop();

    // アクター停止時、watchersリストをクリア（メモリリーク防止）
    self.watchers.lock().clear();

    Ok(())
  }
}
```

### 4. ActorContext拡張

**ファイル**: `modules/actor-core/src/actor_prim/actor_context.rs`

```rust
impl<'a, TB: RuntimeToolbox + 'static> ActorContext<'a, TB> {
  /// 指定したアクターの死活を監視開始
  ///
  /// 監視対象が停止すると、`Actor::on_terminated`が呼ばれる
  ///
  /// # Example
  /// ```rust
  /// let child = ctx.spawn_child(&Props::from_fn(|| ChildActor))?;
  /// ctx.watch(child.actor_ref())?;
  /// ```
  pub fn watch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>> {
    // 監視対象に対してSystemMessage::Watch(自分のPid)を送信
    target.send_system_message(SystemMessage::Watch(self.self_ref().pid()))
  }

  /// 指定したアクターの監視を解除
  ///
  /// # Example
  /// ```rust
  /// ctx.unwatch(child.actor_ref())?;
  /// ```
  pub fn unwatch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>> {
    // 監視対象に対してSystemMessage::Unwatch(自分のPid)を送信
    target.send_system_message(SystemMessage::Unwatch(self.self_ref().pid()))
  }

  /// 子アクターを監視付きでspawn（便利メソッド）
  ///
  /// # Example
  /// ```rust
  /// let child = ctx.spawn_child_watched(&Props::from_fn(|| ChildActor))?;
  /// // 自動的にwatchされている
  /// ```
  pub fn spawn_child_watched(
    &self,
    props: &PropsGeneric<TB>,
  ) -> Result<ChildRefGeneric<TB>, SpawnError> {
    let child = self.spawn_child(props)?;
    self.watch(child.actor_ref())
      .map_err(|e| SpawnError::Other(format!("Failed to watch child: {:?}", e)))?;
    Ok(child)
  }
}
```

### 5. Actorトレイト拡張

**ファイル**: `modules/actor-core/src/actor_prim/actor.rs`

```rust
pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send + Sized + 'static {
  // 既存メソッド
  fn pre_start(&mut self, ctx: &mut ActorContext<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContext<'_, TB>,
    message: AnyMessageView<'_, TB>,
  ) -> Result<(), ActorError>;

  fn post_stop(&mut self, ctx: &mut ActorContext<'_, TB>) -> Result<(), ActorError> {
    Ok(())
  }

  /// 監視対象のアクターが終了したときに呼ばれる
  ///
  /// `watch()`で監視開始したアクターが停止すると、このメソッドが呼ばれる。
  /// デフォルト実装は何もしない。
  ///
  /// # Arguments
  /// - `terminated`: 終了したアクターのPid
  ///
  /// # Example
  /// ```rust
  /// fn on_terminated(
  ///   &mut self,
  ///   ctx: &mut ActorContext<'_, TB>,
  ///   terminated: Pid,
  /// ) -> Result<(), ActorError> {
  ///   if terminated == self.child_pid {
  ///     // 子を再作成
  ///     let new_child = ctx.spawn_child_watched(&self.child_props)?;
  ///     self.child_pid = new_child.pid();
  ///   }
  ///   Ok(())
  /// }
  /// ```
  fn on_terminated(
    &mut self,
    _ctx: &mut ActorContext<'_, TB>,
    _terminated: Pid,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}
```

### 6. SystemState統合

**ファイル**: `modules/actor-core/src/system/system_state.rs`

SystemMessageの処理にWatch/Unwatchを追加:

```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
  pub(crate) fn process_system_message(
    &self,
    pid: Pid,
    message: SystemMessage,
  ) -> Result<(), ActorError> {
    match message {
      SystemMessage::Stop => {
        // 既存の停止処理
      }
      SystemMessage::Suspend => {
        // 既存のサスペンド処理
      }
      SystemMessage::Resume => {
        // 既存のレジューム処理
      }
      SystemMessage::Watch(watcher) => {
        // NEW: 監視者追加
        if let Some(actor_cell) = self.get_actor_cell(pid) {
          actor_cell.handle_watch(watcher);
        }
        Ok(())
      }
      SystemMessage::Unwatch(watcher) => {
        // NEW: 監視者削除
        if let Some(actor_cell) = self.get_actor_cell(pid) {
          actor_cell.handle_unwatch(watcher);
        }
        Ok(())
      }
    }
  }
}
```

### 7. EventStream統合

LifecycleEventのフィルタリングロジック:

```rust
impl EventStreamSubscriber for MyActor {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      match lifecycle.watcher() {
        Some(watcher_pid) if watcher_pid == self.my_pid => {
          // 自分が監視しているアクターの終了
          // Actor::on_terminatedにディスパッチ
          let _ = self.on_terminated(ctx, lifecycle.pid());
        }
        None => {
          // システムワイド観測イベント（ロギング、メトリクスなど）
          self.log_lifecycle_event(lifecycle);
        }
        _ => {
          // 他の監視者向けイベントは無視
        }
      }
    }
  }
}
```

## データフロー

### Watch/Unwatch フロー

```
[ActorContext::watch]
       ↓
SystemMessage::Watch(watcher_pid) 送信
       ↓
[SystemState::process_system_message]
       ↓
[ActorCell::handle_watch]
       ↓
watchers.push(watcher_pid) (冪等)
```

### アクター停止時の通知フロー

```
[ActorCell::stop]
       ↓
[ActorCell::notify_watchers_on_stop]
       ↓
for each watcher in watchers:
  ├─ LifecycleEvent::new_terminated(pid, watcher) 生成
  └─ EventStream::publish(Lifecycle(event))
       ↓
[EventStreamサブスクライバー]
       ↓
if event.watcher() == Some(my_pid):
  ├─ [Actor::on_terminated] 呼び出し
  └─ 子アクター再起動などの処理

システムワイド観測:
  ├─ LifecycleEvent::new_stopped(pid) 生成
  └─ EventStream::publish(Lifecycle(event))
       ↓
[EventStreamサブスクライバー]
       ↓
if event.watcher().is_none():
  └─ ロギング、メトリクス収集
```

## エッジケース処理

### 1. 循環監視（A watches B, B watches A）

**対応**: 許容（Akka/Pekkoと同じ）

理由:
- 循環監視を検出・禁止するコストが高い
- 実用上は問題にならない（停止は一方向に伝播）
- ドキュメントで注意喚起

### 2. 既に停止したアクターをwatch

**対応**: watchは成功するが、通知は来ない

実装:
```rust
pub fn watch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>> {
  // SystemMessage送信が成功すればOk
  // 対象が既に停止している場合、メッセージは配信されないが、
  // それはメールボックスの責任範囲
  target.send_system_message(SystemMessage::Watch(self.self_ref().pid()))
}
```

### 3. 監視者自身が停止した場合

**対応**: 監視対象のwatchersリストから自動削除は**しない**

理由:
- 停止した監視者のPidへのイベント送信は無害（配信失敗するだけ）
- 監視対象が停止時にwatchersをクリアするので、最終的にメモリリークしない

### 4. 同じアクターを複数回watch

**対応**: 冪等（2回目以降は無視）

実装:
```rust
pub(crate) fn handle_watch(&self, watcher: Pid) {
  let mut watchers = self.watchers.lock();
  if !watchers.contains(&watcher) {
    watchers.push(watcher);
  }
}
```

### 5. unwatchしていないアクターをunwatch

**対応**: 無害（何もしない）

実装:
```rust
pub(crate) fn handle_unwatch(&self, watcher: Pid) {
  self.watchers.lock().retain(|pid| *pid != watcher);
}
```

## 実装スケジュール

### Phase 1: コアインフラ構築（破壊的変更）

**対象ファイル**:
- `modules/actor-core/src/lifecycle/lifecycle_event.rs`
- `modules/actor-core/src/messaging/system_message.rs`

**タスク**:
1. LifecycleEventに`watcher: Option<Pid>`フィールド追加
2. `new_started/new_stopped/new_restarted/new_terminated/is_watched/watcher`メソッド追加
3. SystemMessageに`Watch(Pid)/Unwatch(Pid)` variant追加
4. 既存のLifecycleEvent生成箇所を便利メソッドに移行

**完了条件**:
- 全テストがパス（便利メソッドにより最小限の修正）
- ドキュメントが更新される

### Phase 2: ActorCell拡張

**対象ファイル**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

**タスク**:
1. `watchers: ToolboxMutex<Vec<Pid>, TB>`フィールド追加
2. `handle_watch/handle_unwatch`実装
3. `notify_watchers_on_stop`実装
4. `stop`メソッドに`notify_watchers_on_stop`呼び出しを追加

**完了条件**:
- 単体テストでwatcher追加/削除が動作
- 停止時にLifecycleEvent(watcher: Some)が発行される

### Phase 3: API追加

**対象ファイル**:
- `modules/actor-core/src/actor_prim/actor_context.rs`
- `modules/actor-core/src/actor_prim/actor.rs`

**タスク**:
1. `ActorContext::watch/unwatch/spawn_child_watched`実装
2. `Actor::on_terminated`デフォルト実装追加

**完了条件**:
- watch/unwatchが正常に動作
- spawn_child_watchedが子を監視付きで生成

### Phase 4: SystemState統合

**対象ファイル**:
- `modules/actor-core/src/system/system_state.rs`

**タスク**:
1. SystemMessage::Watch/Unwatchの処理追加

**完了条件**:
- SystemMessage経由でwatch/unwatchが動作

### Phase 5: テストと検証

**対象ファイル**:
- `modules/actor-core/tests/death_watch.rs`（新規）

**テストケース**:
1. 基本的な監視: watch → 子停止 → on_terminated呼び出し
2. unwatch後は通知されない
3. 複数監視者が全員通知を受け取る
4. システムワイド観測イベントも発行される
5. 監視者向けと通常イベントの区別（`is_watched()`）
6. 循環監視でもデッドロックしない
7. 既に停止したアクターをwatchしてもエラーにならない
8. 同じアクターを複数回watchしても冪等
9. no_std環境での動作確認

**完了条件**:
- actor-coreの全テストがパス
- actor-stdでも動作確認
- カバレッジ ≥ 90%

### Phase 6: ドキュメントと例

**対象ファイル**:
- `examples/death_watch.rs`（新規）
- `README.md`更新
- APIドキュメント

**タスク**:
1. watch/unwatchの使用例追加
2. Akka/Pekkoからの移行ガイド
3. ベストプラクティス文書化

**完了条件**:
- exampleが正常動作
- ドキュメントが完全

## パフォーマンス分析

### メモリ使用量

- **ActorCell**: `watchers: Vec<Pid>` 追加
  - 監視者0人: 24バイト（Vec容量）
  - 監視者1人: 24バイト + 8バイト = 32バイト
  - 監視者n人: 24 + 8n バイト

- **LifecycleEvent**: `watcher: Option<Pid>` 追加
  - 8バイト（Option<Pid>のサイズ）

### CPU使用量

- **watch/unwatch**: O(1) SystemMessage送信 + O(n) Vec操作（n=監視者数、通常は少数）
- **notify_watchers_on_stop**: O(n) イベント発行（n=監視者数）

### イベント発行数

アクター停止時:
- 監視者がいない: LifecycleEvent(Stopped, watcher=None) × 1
- 監視者がn人: LifecycleEvent(Stopped, watcher=None) × 1 + LifecycleEvent(Stopped, watcher=Some) × n

## セキュリティ考慮事項

1. **メモリリーク防止**: アクター停止時にwatchersをクリア
2. **監視者の認証**: なし（同じActorSystem内なら誰でも監視可能）
3. **情報漏洩**: LifecycleEventは終了理由を含まない（セキュリティ上安全）

## 移行ガイド

### 既存コードへの影響

**破壊的変更**:
- LifecycleEventを直接生成している箇所は`watcher: None`を追加する必要がある

**推奨される移行手順**:
1. LifecycleEvent生成を便利メソッド（`new_started`等）に置き換え
2. パターンマッチングに`..`を使用して新フィールドを無視
3. テストを実行して破壊的変更を検出

**例**:
```rust
// Before
let event = LifecycleEvent {
  pid,
  parent,
  name,
  stage: LifecycleStage::Started,
  timestamp,
};

// After (Option 1: 便利メソッド使用)
let event = LifecycleEvent::new_started(pid, parent, name, timestamp);

// After (Option 2: 直接生成)
let event = LifecycleEvent {
  pid,
  parent,
  name,
  stage: LifecycleStage::Started,
  timestamp,
  watcher: None,
};

// パターンマッチング
match lifecycle {
  LifecycleEvent { pid, stage, .. } => {
    // watcherフィールドを無視
  }
}
```

### Akka/Pekkoからの移行

```scala
// Akka/Pekko
context.watch(child)
context.unwatch(child)

def receive = {
  case Terminated(ref) =>
    // 子の死を処理
}
```

```rust
// cellactor-rs
ctx.watch(child.actor_ref())?;
ctx.unwatch(child.actor_ref())?;

fn on_terminated(&mut self, ctx: &mut ActorContext, terminated: Pid) -> Result<(), ActorError> {
  // 子の死を処理
  Ok(())
}
```

## モジュール間の関係

### actor-core
- 全ての変更はactor-coreに実装される
- no_std環境で動作（組み込みシステム対応）
- RuntimeToolbox抽象化により環境非依存

### actor-std
- actor-coreを再エクスポート
- actor-coreの変更を自動的に継承
- std環境での利用時も同じAPIが利用可能
- 追加の実装は不要（actor-coreの実装がそのまま使える）

### 使用例

```rust
// no_std環境（actor-core）
use cellactor_core::{Actor, ActorContext};

impl Actor for MyActor {
  fn pre_start(&mut self, ctx: &mut ActorContext) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&props)?;
    ctx.watch(child.actor_ref())?;  // ✅ 利用可能
    Ok(())
  }
}

// std環境（actor-std）
use cellactor_std::{Actor, ActorContext};

impl Actor for MyActor {
  fn pre_start(&mut self, ctx: &mut ActorContext) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&props)?;
    ctx.watch(child.actor_ref())?;  // ✅ 同じAPIが利用可能
    Ok(())
  }
}
```

## 関連仕様

- **actor-lifecycle**: LifecycleEventの定義と使用
- **event-stream**: EventStreamEventとサブスクライバー
- **supervision**: SupervisorStrategyとの連携

## 参考資料

- [Akka DeathWatch](https://doc.akka.io/docs/akka/current/actors.html#deathwatch)
- [Pekko Actor Lifecycle](https://pekko.apache.org/docs/pekko/current/typed/actor-lifecycle.html)
