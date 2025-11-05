# 提案: ActorContext watch/unwatch API - DeathWatch-style Actor Monitoring

## Why

### 現状の問題点

cellactor-rsは現在、EventStream経由でアクターのライフサイクルイベント（Started/Restarted/Stopped）を監視できますが、Akka/Pekkoの`DeathWatch`（`context.watch()`）のような**個別アクターの明示的な監視API**が不足しています。

#### 問題1: 監視の冗長性
```rust
// 現状: 全てのLifecycleイベントを受信してフィルタリングが必要
impl EventStreamSubscriber for ParentActor {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      // 手動で特定Pidをフィルタリング
      if lifecycle.pid() == target_pid && lifecycle.stage() == LifecycleStage::Stopped {
        // 処理
      }
    }
  }
}
```

#### 問題2: アクターロジックとの分離
- EventStreamサブスクライバーは`Actor::receive`ハンドラーの外で動作
- 子アクターの死活監視がアクター本体のロジックから分離されている
- Akka/Pekkoユーザーにとって直感的ではない

#### 問題3: Akka/Pekkoからの移行障壁
```scala
// Akka
context.watch(child)
def receive = {
  case Terminated(ref) => // 子の死を処理
}
```
この明示的で簡潔なパターンがcellactor-rsでは実現できない。

### 期待される効果

1. **Akka互換性向上**: `context.watch()/unwatch()`で移行が容易に
2. **コードの簡潔化**: 個別監視により不要なフィルタリングロジックを削減
3. **アクターロジックの統合**: `Actor::on_terminated`で監視処理をアクター内に統合
4. **柔軟性の向上**: EventStreamとwatch APIの両方を使い分け可能

## What Changes

### 新規追加コンポーネント

#### 1. SystemMessageの拡張
```rust
// modules/actor-core/src/messaging/system_message.rs
pub enum SystemMessage {
  Stop,
  Suspend,
  Resume,
  Watch(Pid),    // NEW: 監視者のPid
  Unwatch(Pid),  // NEW: 監視解除
}
```

#### 2. LifecycleEventの拡張（破壊的変更）
```rust
// modules/actor-core/src/lifecycle/lifecycle_event.rs
/// ライフサイクルイベント - アクターの状態遷移を表現
#[derive(Clone, Debug)]
pub struct LifecycleEvent {
  pid: Pid,
  parent: Option<Pid>,
  name: String,
  stage: LifecycleStage,
  timestamp: Duration,
  watcher: Option<Pid>,  // NEW: 監視者のPid（watch専用イベントの場合のみ）
}

impl LifecycleEvent {
  // 既存コード向けの便利メソッド
  pub fn new_started(pid: Pid, parent: Option<Pid>, name: String, timestamp: Duration) -> Self;
  pub fn new_restarted(pid: Pid, parent: Option<Pid>, name: String, timestamp: Duration) -> Self;
  pub fn new_stopped(pid: Pid, parent: Option<Pid>, name: String, timestamp: Duration) -> Self;

  // NEW: 監視者向け終了通知イベント生成
  pub fn new_terminated(pid: Pid, parent: Option<Pid>, name: String, timestamp: Duration, watcher: Pid) -> Self;

  // NEW: このイベントが監視者向けかどうかを判定
  pub fn is_watched(&self) -> bool {
    self.watcher.is_some()
  }
}
```

**設計の理由**:
- 単一の事象（アクター終了）を単一のイベント型で表現（DRY原則）
- `watcher: None` → システムワイドの観測可能性イベント（全EventStreamサブスクライバー向け）
- `watcher: Some(pid)` → 特定の監視者向けDeathWatchイベント
- EventStreamEventに新しいvariantを追加する必要がない

#### 3. ActorCellの拡張
```rust
// modules/actor-core/src/actor_prim/actor_cell.rs
pub struct ActorCellGeneric<TB: RuntimeToolbox, A: Actor<TB>> {
  // 既存フィールド
  pid: Pid,
  actor: ToolboxMutex<A, TB>,
  children: ToolboxMutex<Vec<Pid>, TB>,

  // NEW: このアクターを監視している親/アクターのリスト
  watchers: ToolboxMutex<Vec<Pid>, TB>,
}

impl ActorCellGeneric {
  // NEW: Watchメッセージ処理
  pub(crate) fn handle_watch(&self, watcher: Pid);

  // NEW: Unwatchメッセージ処理
  pub(crate) fn handle_unwatch(&self, watcher: Pid);

  // NEW: Stopped時に監視者に通知
  fn notify_watchers_on_stop(&self);
}
```

#### 4. ActorContextの拡張
```rust
// modules/actor-core/src/actor_prim/actor_context.rs
impl<'a, TB: RuntimeToolbox + 'static> ActorContext<'a, TB> {
  /// 指定したアクターの死活を監視開始
  pub fn watch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>>;

  /// 指定したアクターの監視を解除
  pub fn unwatch(&self, target: &ActorRefGeneric<TB>) -> Result<(), SendError<TB>>;

  /// 子アクターを監視付きでspawn（便利メソッド）
  pub fn spawn_child_watched(&self, props: &PropsGeneric<TB>)
    -> Result<ChildRefGeneric<TB>, SpawnError>;
}
```

#### 5. Actorトレイトの拡張
```rust
// modules/actor-core/src/actor_prim/actor.rs
pub trait Actor<TB: RuntimeToolbox = NoStdToolbox>: Send + Sized + 'static {
  // 既存メソッド
  fn pre_start(&mut self, ctx: &mut ActorContext<'_, TB>) -> Result<(), ActorError>;
  fn receive(&mut self, ctx: &mut ActorContext<'_, TB>, message: AnyMessageView<'_, TB>)
    -> Result<(), ActorError>;
  fn post_stop(&mut self, ctx: &mut ActorContext<'_, TB>) -> Result<(), ActorError>;

  /// NEW: 監視対象のアクターが終了したときに呼ばれる
  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_, TB>, _terminated: Pid)
    -> Result<(), ActorError> {
    Ok(())  // デフォルトは何もしない
  }
}
```

### 使用例

#### Before (現在)
```rust
// EventStreamサブスクライバーを別途実装
struct ParentSubscriber {
  child_pid: Pid,
}

impl EventStreamSubscriber for ParentSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      if lifecycle.pid() == self.child_pid && lifecycle.stage() == LifecycleStage::Stopped {
        // 子の死を処理（アクター外）
      }
    }
  }
}
```

#### After (提案)
```rust
// アクター内で完結
struct ParentActor {
  child_ref: Option<ChildRef>,
}

impl Actor for ParentActor {
  fn pre_start(&mut self, ctx: &mut ActorContext) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&Props::from_fn(|| ChildActor))?;
    ctx.watch(child.actor_ref())?;  // 監視開始
    self.child_ref = Some(child);
    Ok(())
  }

  fn on_terminated(&mut self, ctx: &mut ActorContext, terminated: Pid)
    -> Result<(), ActorError> {
    if let Some(ref child) = self.child_ref {
      if child.pid() == terminated {
        // 子を再作成
        let new_child = ctx.spawn_child_watched(&Props::from_fn(|| ChildActor))?;
        self.child_ref = Some(new_child);
      }
    }
    Ok(())
  }
}
```

## Impact

### 破壊的変更

**BREAKING CHANGE**: `LifecycleEvent`構造体に`watcher: Option<Pid>`フィールドを追加

**影響範囲**:
- `LifecycleEvent`を直接生成している箇所で`watcher`フィールドの指定が必要
- 既存の構造体初期化は`watcher: None`を追加することで対応可能
- パターンマッチングで全フィールドを列挙している箇所は修正が必要

**緩和策**:
- `LifecycleEvent::new_started/new_stopped/new_restarted`便利メソッドを提供
- これらのメソッドは`watcher: None`をデフォルトで設定
- 既存コードはこれらのメソッドを使うことで最小限の変更で済む

### 影響を受けるコンポーネント

- **Affected specs**: actor-lifecycle, event-stream
- **Affected modules**:
  - `modules/actor-core` - コア実装（主要変更箇所）
  - `modules/actor-std` - actor-coreの変更を継承（自動的に利用可能）
- **Affected code**:
  - `modules/actor-core/src/messaging/system_message.rs` - SystemMessage拡張
  - `modules/actor-core/src/lifecycle/lifecycle_event.rs` - watcherフィールド追加（破壊的変更）
  - `modules/actor-core/src/actor_prim/actor_cell.rs` - watchersフィールド追加
  - `modules/actor-core/src/actor_prim/actor_context.rs` - watch/unwatch API追加
  - `modules/actor-core/src/actor_prim/actor.rs` - on_terminated追加
  - `modules/actor-core/src/system/system_state.rs` - Watch/Unwatch処理追加
  - 既存のテストコード - LifecycleEvent生成箇所の修正

### 実装の段階

#### Phase 1: コアインフラ構築（破壊的変更含む）
1. `LifecycleEvent`に`watcher: Option<Pid>`フィールド追加
2. `LifecycleEvent::new_started/new_stopped/new_restarted/new_terminated`メソッド追加
3. 既存のLifecycleEvent生成箇所を便利メソッドに移行
4. `SystemMessage::Watch/Unwatch`追加
5. `ActorCell`に`watchers`フィールド追加
6. `ActorCell::handle_watch/unwatch/notify_watchers_on_stop`実装

#### Phase 2: API追加
1. `ActorContext::watch/unwatch`実装
2. `ActorContext::spawn_child_watched`実装
3. `Actor::on_terminated`デフォルト実装追加

#### Phase 3: システム統合
1. `SystemState`でWatch/Unwatchメッセージ処理
2. `ActorCell::stop`時に`notify_watchers_on_stop`を呼び出し
3. `notify_watchers_on_stop`内で監視者ごとに`LifecycleEvent { watcher: Some(pid) }`を発行
4. 通常のLifecycleEvent(Stopped)は`watcher: None`で発行（既存の観測可能性維持）

#### Phase 4: テストと検証
1. 基本的な監視テスト（watch → 子停止 → on_terminated呼び出し）
2. unwatch後は通知されないことの検証
3. 複数監視者のテスト
4. EventStream統合テスト

### 互換性と移行

#### 既存コードへの影響
- **破壊的変更**: LifecycleEvent構造体のフィールド追加により、直接生成している箇所は修正が必要
- **緩和策**: 便利メソッド(`new_started`等)を使えば最小限の変更で対応可能
- **観測可能性の維持**: 既存のEventStreamベースの監視は`watcher: None`として引き続き動作
- **追加オプション**: 新しいwatch APIは追加の選択肢として提供

#### Akka/Pekkoからの移行
```scala
// Akka/Pekko
context.watch(child)
context.unwatch(child)
case Terminated(ref) => ...
```
↓
```rust
// cellactor-rs
ctx.watch(child.actor_ref())?;
ctx.unwatch(child.actor_ref())?;
fn on_terminated(&mut self, ctx, pid) { ... }
```

### メモリオーバーヘッド

- **ActorCell**: `watchers: Vec<Pid>`を1つ追加（通常は数要素程度）
- **LifecycleEvent**: `watcher: Option<Pid>`フィールド追加（8バイト）
- **イベント発行数**: 監視者がいる場合、停止時に`1 + 監視者数`個のLifecycleEventが発行される
  - 通常のLifecycleEvent(Stopped) × 1（システムワイド観測用）
  - 監視者向けLifecycleEvent(Stopped) × 監視者数
- **no_std対応**: 全ての新規コンポーネントはno_std環境でも動作

### パフォーマンス考慮事項

1. **Watch/Unwatch**: SystemMessage送信のみ、O(1)操作
2. **監視者リスト管理**: Vec操作、O(n) (nは監視者数、通常は少数)
3. **通知**: 監視者数に比例、O(n)
4. **EventStream**: 既存の仕組みを利用、追加オーバーヘッドなし

### 成功基準

1. ✅ `ActorContext::watch/unwatch` APIが動作する
2. ✅ `Actor::on_terminated`が子アクター停止時に呼ばれる
3. ✅ unwatchしたアクターからは通知が来ない
4. ✅ 複数の監視者が全員通知を受け取る（各監視者向けに`watcher: Some(pid)`イベント発行）
5. ✅ システムワイドの観測可能性が維持される（`watcher: None`イベント発行）
6. ✅ 監視者向けイベントと通常イベントが明確に区別できる（`is_watched()`メソッド）
7. ✅ 既存のテストが全てパスする（便利メソッド使用により最小限の修正）
8. ✅ no_std環境（actor-core）で動作する
9. ✅ std環境（actor-std）でも動作する
10. ✅ examplesにwatch/unwatchを使った例が追加される

### リスクと緩和策

**リスク1**: メモリリーク（unwatchし忘れ）
- **緩和策**: アクター停止時に自動的に監視者リストをクリア

**リスク2**: 循環監視（A watches B, B watches A）
- **緩和策**: ドキュメントで注意喚起、実装上は許容（Akkaと同様）

**リスク3**: パフォーマンス影響
- **緩和策**: 監視者リストはVec（小規模なら高速）、必要なら最適化

### 関連資料

- Akka DeathWatch: https://doc.akka.io/docs/akka/current/actors.html#deathwatch
- Pekko Lifecycle: https://pekko.apache.org/docs/pekko/current/typed/actor-lifecycle.html
- 既存のEventStream実装: `modules/actor-core/src/event_stream/`
- 既存のSystemMessage: `modules/actor-core/src/messaging/system_message.rs`
