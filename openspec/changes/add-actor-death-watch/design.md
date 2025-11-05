# 設計書: ActorContext watch/unwatch API - DeathWatch実装

## アーキテクチャ概要

### 設計原則

**SystemMessage直接送信方式**:
- EventStreamを経由せず、監視者のメールボックスに直接`SystemMessage::Terminated`を送信
- Akka/Pekkoと同じ「システムメッセージによるDeathWatch」の仕組み
- 順序保証: システムメッセージは通常メッセージより優先処理
- O(n)の効率: 監視者数に比例、全サブスクライバーへのブロードキャスト不要

### なぜSystemMessage方式か

#### EventStream方式の問題点（レビュー指摘事項）
```rust
// ❌ EventStream方式: 実装不可能
impl EventStreamSubscriber for MyActor {
  fn on_event(&self, event: &EventStreamEvent) {
    // 問題1: `&self`しかなく、`&mut ActorContext`が取得できない
    // 問題2: EventStreamはアクター処理スレッド外で動作
    // 問題3: 全サブスクライバーにブロードキャストされる（O(n×m)）
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      let _ = self.on_terminated(ctx, lifecycle.pid());  // ❌ ctxがない！
    }
  }
}
```

**問題点**:
1. **EventStreamSubscriberは`&self`のみ**: `&mut ActorContext`を取得できない
2. **スレッド安全性の欠如**: EventStreamはアクター処理外で動く
3. **ブロードキャストオーバーヘッド**: 監視者数×サブスクライバー数のイベント処理
4. **順序保証なし**: EventStreamの配送順序に依存し、通常メッセージより後に処理される可能性

#### SystemMessage方式の利点
```rust
// ✅ SystemMessage方式: 実装可能で効率的
// ActorCell::handle_terminated (システムメッセージ処理コンテキスト内)
pub(crate) fn handle_terminated(&self, terminated_pid: Pid) -> Result<(), ActorError> {
  let system = ActorSystemGeneric::from_state(self.system.clone());
  let mut ctx = ActorContext::new(&system, self.pid);
  let mut actor = self.actor.lock();

  // ✅ ActorCellが&mut ActorContextを用意して呼び出す
  actor.on_terminated(&mut ctx, terminated_pid)
}
```

**利点**:
1. **コンテキスト確保**: ActorCellがActorContextを作成し、`on_terminated`に渡す
2. **アクター処理内**: 通常のシステムメッセージ処理と同じフロー
3. **O(n)効率**: 監視者のみに送信
4. **順序保証**: SystemMessageは優先処理される

## コンポーネント設計

### 1. SystemMessage拡張

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

  /// 監視対象のアクターが停止したことを通知
  /// ペイロードは停止したアクターのPid
  Terminated(Pid),
}
```

### 2. ActorCell拡張

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

  /// 停止時に監視者にSystemMessage::Terminatedを送信
  fn notify_watchers_on_stop(&self) {
    let watchers = self.watchers.lock().clone();

    // 各監視者のメールボックスに直接SystemMessage::Terminatedを送信
    for watcher_pid in watchers {
      // SystemMessage送信（EventStreamは経由しない）
      let _ = self.system.send_system_message(
        watcher_pid,
        SystemMessage::Terminated(self.pid)
      );
    }

    // システムワイド観測用のLifecycleEvent(Stopped)は従来通り発行
    let timestamp = self.system.monotonic_now();
    let system_event = LifecycleEvent::new_stopped(
      self.pid,
      self.parent,
      self.pid.name().to_string(),
      timestamp,
    );
    self.system.publish_event(EventStreamEvent::Lifecycle(system_event));
  }

  /// SystemMessage::Terminatedを受信したときの処理
  pub(crate) fn handle_terminated(&self, terminated_pid: Pid) -> Result<(), ActorError> {
    // ActorSystemとActorContextを用意
    let system = ActorSystemGeneric::from_state(self.system.clone());
    let mut ctx = ActorContext::new(&system, self.pid);

    // Actorをロックしてon_terminatedを呼び出す
    let mut actor = self.actor.lock();
    actor.on_terminated(&mut ctx, terminated_pid)
    // ここで&mut ActorContextが利用可能
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

### 3. ActorContext拡張

**ファイル**: `modules/actor-core/src/actor_prim/actor_context.rs`

```rust
impl<'a, TB: RuntimeToolbox + 'static> ActorContext<'a, TB> {
  /// 指定したアクターの死活を監視開始
  ///
  /// 監視対象が停止すると、`Actor::on_terminated`が呼ばれる
  ///
  /// # 既に停止している場合の挙動
  /// 対象アクターが既に停止している場合、即座にSystemMessage::Terminatedが送信される（Akka互換）
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

### 4. Actorトレイト拡張

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
  /// # 呼び出しコンテキスト
  /// このメソッドは、SystemMessage::Terminatedを受信したActorCellが
  /// `handle_terminated()`内で呼び出す。通常のアクター処理コンテキスト内で
  /// 実行されるため、`&mut ActorContext`が利用可能。
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

### 5. SystemState統合

**ファイル**: `modules/actor-core/src/system/system_state.rs`

SystemMessageの処理にWatch/Unwatch/Terminatedを追加:

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

          // 重要: 対象アクターが既に停止している場合、即座にTerminatedを送信
          if !self.is_actor_alive(pid) {
            let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
          }
        } else {
          // アクターが存在しない（既に停止）→ 即座にTerminatedを送信
          let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
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
      SystemMessage::Terminated(terminated_pid) => {
        // NEW: 終了通知処理
        if let Some(actor_cell) = self.get_actor_cell(pid) {
          actor_cell.handle_terminated(terminated_pid)?;
        }
        Ok(())
      }
    }
  }

  /// SystemMessageを送信
  pub(crate) fn send_system_message(
    &self,
    target: Pid,
    message: SystemMessage,
  ) -> Result<(), SendError<TB>> {
    // メールボックスにシステムメッセージを送信
    // （システムメッセージは通常メッセージより優先処理される）
  }

  /// アクターが生存しているか確認
  fn is_actor_alive(&self, pid: Pid) -> bool {
    self.get_actor_cell(pid).is_some()
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
       ├─ アクターが生存している場合
       │  ├─ [ActorCell::handle_watch]
       │  └─ watchers.push(watcher_pid) (冪等)
       └─ アクターが既に停止している場合
          └─ SystemMessage::Terminated(target_pid)を即座に送信 ★重要
```

### アクター停止時の通知フロー

```
[ActorCell::stop]
       ↓
[ActorCell::notify_watchers_on_stop]
       ↓
for each watcher in watchers:
  └─ SystemMessage::Terminated(self.pid)を送信
     ↓
  [監視者のメールボックス]
     ↓
  [SystemState::process_system_message]
     ↓
  [ActorCell::handle_terminated]
     ├─ let system = ActorSystemGeneric::from_state(self.system.clone());
     ├─ let mut ctx = ActorContext::new(&system, self.pid);
     ├─ let mut actor = self.actor.lock();
     └─ actor.on_terminated(&mut ctx, terminated_pid); ★ここで呼び出し
         ↓
     [Actor::on_terminated]
         └─ ユーザーコード（子の再起動など）

システムワイド観測:
  └─ LifecycleEvent::new_stopped(pid)を発行
     ↓
  [EventStream::publish]
     ↓
  [全EventStreamサブスクライバー]
     └─ ロギング、メトリクス収集
```

### 既に停止したアクターをwatchした場合のフロー

```
[ActorContext::watch(既に停止したアクター)]
       ↓
SystemMessage::Watch(watcher_pid) 送信
       ↓
[SystemState::process_system_message]
       ↓
  アクターが存在しない（既に停止）を検出
       ↓
  SystemMessage::Terminated(target_pid)を即座に送信 ★Akka互換
       ↓
[監視者のメールボックス]
       ↓
[ActorCell::handle_terminated]
       ↓
[Actor::on_terminated]
       └─ 即座に終了通知を受け取る
```

## エッジケース処理

### 1. 循環監視（A watches B, B watches A）

**対応**: 許容（Akka/Pekkoと同じ）

理由:
- 循環監視を検出・禁止するコストが高い
- 実用上は問題にならない（停止は一方向に伝播）
- ドキュメントで注意喚起

### 2. 既に停止したアクターをwatch

**対応**: 即座にSystemMessage::Terminatedを送信（Akka互換）

実装:
```rust
SystemMessage::Watch(watcher) => {
  if let Some(actor_cell) = self.get_actor_cell(pid) {
    actor_cell.handle_watch(watcher);
    if !self.is_actor_alive(pid) {
      let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
    }
  } else {
    // アクターが存在しない → 即座にTerminatedを送信
    let _ = self.send_system_message(watcher, SystemMessage::Terminated(pid));
  }
}
```

### 3. 監視者自身が停止した場合

**対応**: 監視対象のwatchersリストから自動削除は**しない**

理由:
- 停止した監視者へのSystemMessage送信は無害（配信失敗するだけ）
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

### Phase 1: コアインフラ構築

**対象ファイル**:
- `modules/actor-core/src/messaging/system_message.rs`

**タスク**:
1. SystemMessageに`Watch(Pid)/Unwatch(Pid)/Terminated(Pid)` variant追加

**完了条件**:
- コンパイルが通る
- 単体テストがパス

### Phase 2: ActorCell拡張

**対象ファイル**:
- `modules/actor-core/src/actor_prim/actor_cell.rs`

**タスク**:
1. `watchers: ToolboxMutex<Vec<Pid>, TB>`フィールド追加
2. `handle_watch/handle_unwatch`実装
3. `notify_watchers_on_stop`実装（SystemMessage::Terminated送信）
4. `handle_terminated`実装（Actor::on_terminatedを呼び出す）
5. `stop`メソッドに`notify_watchers_on_stop`呼び出しを追加

**完了条件**:
- 単体テストでwatcher追加/削除が動作
- 停止時にSystemMessage::Terminatedが送信される
- handle_terminatedでon_terminatedが呼ばれる

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
1. SystemMessage::Watch/Unwatch/Terminatedの処理追加
2. 既に停止したアクターをwatchした場合の即座のTerminated送信

**完了条件**:
- SystemMessage経由でwatch/unwatchが動作
- 既に停止したアクターをwatchすると即座にon_terminatedが呼ばれる

### Phase 5: テストと検証

**対象ファイル**:
- `modules/actor-core/tests/death_watch.rs`（新規）

**テストケース**:
1. 基本的な監視: watch → 子停止 → on_terminated呼び出し
2. unwatch後は通知されない
3. 複数監視者が全員通知を受け取る
4. 既に停止したアクターをwatchすると即座にon_terminatedが呼ばれる（★重要）
5. 循環監視でもデッドロックしない
6. 同じアクターを複数回watchしても冪等
7. 順序保証: on_terminatedが通常メッセージより優先処理される
8. no_std環境での動作確認

**完了条件**:
- 全テストがパス
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

- **SystemMessage**: Terminated(Pid) variant追加
  - enum全体のサイズは変わらない（最大variantに合わせる）

### CPU使用量

- **watch/unwatch**: O(1) SystemMessage送信 + O(n) Vec操作（n=監視者数、通常は少数）
- **notify_watchers_on_stop**: O(n) SystemMessage送信（n=監視者数）
- **EventStream方式との比較**: O(n) vs O(n×m) (m=全サブスクライバー数)

### メッセージ発行数

アクター停止時:
- 監視者がいない: LifecycleEvent(Stopped) × 1（従来通り）
- 監視者がn人: SystemMessage::Terminated × n + LifecycleEvent(Stopped) × 1

**EventStream方式との比較**:
- EventStream方式: n+1個のLifecycleEventを全サブスクライバーに配信
- SystemMessage方式: n個のSystemMessageを監視者のみに送信 + 1個のLifecycleEventを全サブスクライバーに配信

## セキュリティ考慮事項

1. **メモリリーク防止**: アクター停止時にwatchersをクリア
2. **監視者の認証**: なし（同じActorSystem内なら誰でも監視可能）
3. **情報漏洩**: SystemMessage::Terminatedは停止Pidのみ、終了理由を含まない（セキュリティ上安全）

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

  fn on_terminated(&mut self, ctx: &mut ActorContext, terminated: Pid) -> Result<(), ActorError> {
    // 子を再作成
    let new_child = ctx.spawn_child_watched(&props)?;
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

- **actor-lifecycle**: Actorのライフサイクル管理
- **system-messaging**: SystemMessageの定義と処理
- **supervision**: SupervisorStrategyとの連携

## 参考資料

- [Akka DeathWatch](https://doc.akka.io/docs/akka/current/actors.html#deathwatch)
- [Pekko Actor Lifecycle](https://pekko.apache.org/docs/pekko/current/typed/actor-lifecycle.html)
