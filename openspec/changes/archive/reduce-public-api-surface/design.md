# 設計: actor-coreの公開API表面積削減

## アーキテクチャ概要

### 現在のアーキテクチャ

```
┌─────────────────────────────────────────┐
│         ユーザーアプリケーション           │
└─────────────────┬───────────────────────┘
                  │
    ┌─────────────┴─────────────┐
    │                           │
    v                           v
┌─────────┐              ┌─────────────┐
│actor-std│              │  actor-core │
│(ラッパー)│◄─────────────┤  (直接使用) │
└────┬────┘              └─────────────┘
     │
     v
┌──────────────────────────────────────────┐
│         actor-core (実装)                │
│  ┌────────────────────────────────────┐  │
│  │ SystemStateGeneric (27 public)    │  │
│  │ DispatcherGeneric (9 public)      │  │
│  │ MailboxGeneric (12 public)        │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

**問題点**:
- actor-coreを直接使用する場合、内部実装メソッドも公開されている
- APIの境界が不明確
- 内部実装の変更が外部に影響するリスク

### 提案するアーキテクチャ

```
┌─────────────────────────────────────────┐
│         ユーザーアプリケーション           │
└─────────────────┬───────────────────────┘
                  │
                  │ (推奨パス)
                  v
              ┌─────────┐
              │actor-std│
              │(ラッパー)│
              └────┬────┘
                   │
                   v
┌──────────────────────────────────────────┐
│         actor-core (実装)                │
│  ┌────────────────────────────────────┐  │
│  │ 🔓 公開API (12個)                 │  │
│  │  - new, allocate_pid, etc.       │  │
│  ├────────────────────────────────────┤  │
│  │ 🔒 内部実装 (36個) pub(crate)     │  │
│  │  - register_cell, remove_cell... │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

**改善点**:
- 明確なAPI境界
- 内部実装の隠蔽
- actor-std経由での安全な使用

## コンポーネント設計

### SystemStateGeneric

#### 公開API（維持）

```rust
pub struct SystemStateGeneric<TB: RuntimeToolbox> {
    // 内部フィールド...
}

impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    // ✅ 公開API - ユーザー向け
    pub fn new() -> Self { ... }
    pub fn allocate_pid(&self) -> Pid { ... }
    pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> { ... }
    pub fn dead_letters(&self) -> Vec<DeadLetterEntryGeneric<TB>> { ... }
    pub fn publish_event(&self, event: &EventStreamEvent<TB>) { ... }
    pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) { ... }
    pub fn is_terminated(&self) -> bool { ... }
    pub fn monotonic_now(&self) -> Duration { ... }
    pub fn user_guardian_pid(&self) -> Option<Pid> { ... }
}
```

#### 内部実装（pub(crate)化）

```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    // 🔒 内部実装 - actor-core内部でのみ使用

    // Phase 1: セル管理
    pub(crate) fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) { ... }
    pub(crate) fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }
    pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }

    // Phase 1: システムメッセージ
    pub(crate) fn send_system_message(&self, pid: Pid, msg: SystemMessage) -> Result<(), SendError<TB>> { ... }
    pub(crate) fn notify_failure(&self, pid: Pid, error: ActorError) { ... }
    pub(crate) fn mark_terminated(&self) { ... }

    // Phase 2: 名前管理
    pub(crate) fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> { ... }
    pub(crate) fn release_name(&self, parent: Option<Pid>, name: &str) { ... }

    // Phase 2: ガーディアン管理
    pub(crate) fn set_user_guardian(&self, pid: Pid) { ... }
    pub(crate) fn clear_guardian(&self, pid: Pid) -> bool { ... }
    pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }

    // Phase 2: 子管理
    pub(crate) fn register_child(&self, parent: Pid, child: Pid) { ... }
    pub(crate) fn unregister_child(&self, parent: &Pid, child: &Pid) { ... }
    pub(crate) fn child_pids(&self, parent: Pid) -> Vec<Pid> { ... }

    // Phase 3: Future管理
    pub(crate) fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) { ... }
    pub(crate) fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> { ... }
    pub(crate) fn record_send_error(&self, sender: Option<Pid>, error: &SendError<TB>) { ... }
    pub(crate) fn termination_future(&self) -> ActorFuture<(), TB> { ... }
}
```

### DispatcherGeneric

#### 公開API（維持）

```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    // ✅ 公開API
    pub fn new(mailbox: ArcShared<MailboxGeneric<TB>>, executor: ArcShared<dyn DispatchExecutor<TB>>) -> Self { ... }
    pub fn with_inline_executor(mailbox: ArcShared<MailboxGeneric<TB>>) -> Self { ... }
}
```

#### 内部実装（pub(crate)化）

```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    // 🔒 内部実装 - Phase 1
    pub(crate) fn register_invoker(&self, invoker: ArcShared<dyn MessageInvoker<TB>>) { ... }
    pub(crate) fn enqueue_user(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> { ... }
    pub(crate) fn enqueue_system(&self, message: SystemMessage) { ... }
    pub(crate) fn schedule(&self) { ... }
    pub(crate) fn mailbox(&self) -> ArcShared<MailboxGeneric<TB>> { ... }
    pub(crate) fn create_waker(&self) -> Waker { ... }
    pub(crate) fn into_sender(self: ArcShared<Self>) -> ArcShared<dyn ActorRefSender<TB>> { ... }
}
```

### MailboxGeneric

#### 公開API（維持）

```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    // ✅ 公開API
    pub fn new(policy: MailboxPolicy) -> Self { ... }
}
```

#### 内部実装（pub(crate)化）

```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    // 🔒 内部実装 - Phase 1
    pub(crate) fn set_instrumentation(&self, instr: ArcShared<dyn MailboxInstrumentation<TB>>) { ... }
    pub(crate) fn enqueue_system(&self, msg: SystemMessage) { ... }
    pub(crate) fn enqueue_user(&self, msg: AnyMessageGeneric<TB>) -> EnqueueOutcome { ... }
    pub(crate) fn enqueue_user_future(&self, msg: AnyMessageGeneric<TB>) -> MailboxOfferFuture<TB> { ... }
    pub(crate) fn poll_user_future(&self, cx: &mut Context<'_>) -> Poll<MailboxPollFuture<TB>> { ... }
    pub(crate) fn dequeue(&self) -> Option<MailboxMessage<TB>> { ... }
    pub(crate) fn suspend(&self) { ... }
    pub(crate) fn resume(&self) { ... }

    // 🔒 内部実装 - Phase 3（テスト用）
    pub(crate) fn is_suspended(&self) -> bool { ... }
    pub(crate) fn user_len(&self) -> usize { ... }
    pub(crate) fn system_len(&self) -> usize { ... }
}
```

## データフロー

### メッセージ送信フロー

```
ユーザーコード
    │
    v
ActorRef::tell()  ──────────────────┐
    │                              │ 公開API
    v                              │
ActorRefSender::send()             │
    │                              │
    v                              │
DispatcherSender::send() ──────────┘
    │
    │ ← この境界より下は pub(crate)
    v
Dispatcher::enqueue_user() 🔒
    │
    v
Mailbox::enqueue_user() 🔒
    │
    v
MailboxQueue (内部キュー)
```

### アクタ生成フロー

```
ユーザーコード
    │
    v
ActorSystem::spawn() ──────────────┐
    │                             │ 公開API
    v                             │
ActorSystem::spawn_child() ────────┘
    │
    │ ← この境界より下は pub(crate)
    v
SystemState::register_cell() 🔒
    │
    v
SystemState::assign_name() 🔒
    │
    v
SystemState::register_child() 🔒
```

## 段階的移行戦略

### Phase 1の影響分析

**変更箇所**: 21メソッド

**リスク評価**:
- 低リスク: これらのメソッドはactor-core内部でのみ使用
- actor-stdへの影響: なし（ラッパー経由で同じ機能を提供）

**テスト戦略**:
1. 各メソッド変更後に単体テストを実行
2. actor-core全体のテストを実行
3. actor-stdのテストを実行（回帰テスト）
4. examplesの実行確認

### Phase 2の影響分析

**変更箇所**: 8メソッド（名前管理・子管理）

**リスク評価**:
- 中リスク: spawn/terminateの内部実装に関連
- 慎重な確認が必要

**テスト戦略**:
1. spawn/terminateの統合テストを重点的に実行
2. 名前解決のテストを実行
3. 親子関係のテストを実行

### Phase 3の影響分析

**変更箇所**: 7メソッド（Future管理・テスト用）

**リスク評価**:
- 低〜中リスク: askパターンとテストヘルパー
- テストコードへの影響の可能性

**テスト戦略**:
1. askパターンのテストを重点的に実行
2. テストヘルパーが必要な箇所を特定
3. 必要に応じて`#[cfg(test)] pub`を検討

## パフォーマンスへの影響

### コンパイル時間

**期待される改善**:
- 公開APIの削減により、変更時の再コンパイル範囲が縮小
- 推定削減: 10-15%（公開メソッド数75%削減による）

### ランタイムパフォーマンス

**影響**: なし
- `pub`と`pub(crate)`はランタイムには影響しない
- コンパイラの最適化には影響なし

## セキュリティへの影響

### 脅威モデル

**現状のリスク**:
1. 内部実装の直接操作による不正な状態遷移
2. 意図しない内部APIの使用によるメモリ安全性の問題

**緩和策**:
1. 内部実装を`pub(crate)`化することで、外部からのアクセスを防止
2. actor-std経由での使用を推奨することで、安全なAPIのみを公開

## 代替案との比較

### 代替案1: 全て公開のまま維持

**メリット**:
- 破壊的変更なし
- 最大の柔軟性

**デメリット**:
- APIの肥大化
- 内部実装の変更が困難
- ドキュメントが複雑

**結論**: 不採用（保守性の問題）

### 代替案2: 完全な再設計

**メリット**:
- 理想的なAPI設計が可能

**デメリット**:
- 大規模な破壊的変更
- 実装コストが高い
- リスクが高い

**結論**: 不採用（コスト対効果が低い）

### 採用案: 段階的なpub(crate)化

**メリット**:
- 破壊的変更を最小化
- 段階的な移行が可能
- リスクを管理可能

**デメリット**:
- セマンティックバージョニングのメジャーアップが必要

**結論**: 採用（最良のバランス）

## テスト戦略

### 単体テスト

各Phaseで以下を実行:

```bash
# SystemStateGenericのテスト
cargo test -p cellactor-actor-core-rs system_state

# DispatcherGenericのテスト
cargo test -p cellactor-actor-core-rs dispatcher

# MailboxGenericのテスト
cargo test -p cellactor-actor-core-rs mailbox
```

### 統合テスト

```bash
# actor-core全体
cargo test -p cellactor-actor-core-rs

# actor-std（回帰テスト）
cargo test -p cellactor-actor-std-rs

# 全パッケージ
cargo test --workspace
```

### E2Eテスト

```bash
# examples実行
cargo run --example ping_pong_no_std
cargo run --example deadletter
cargo run --example supervision
cargo run --example named_actor
```

### CI/CD

```bash
# 完全なCIチェック
./scripts/ci-check.sh all
```

## ドキュメント戦略

### 内部ドキュメント

`pub(crate)`メソッドには、内部実装の詳細を記載:

```rust
/// アクタセルをシステムに登録する（内部実装）
///
/// # 注意
///
/// このメソッドは内部実装の詳細であり、直接使用しないでください。
/// アクタの生成には`ActorSystem::spawn()`を使用してください。
pub(crate) fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) {
    // ...
}
```

### 公開ドキュメント

`cargo doc`で生成されるドキュメントから、内部実装が自動的に除外される:

```bash
cargo +nightly doc --no-deps -p cellactor-actor-core-rs
```

### 移行ガイド

`MIGRATION.md`に以下を記載:

```markdown
## v0.x.x → v1.0.0

### 破壊的変更: 内部実装メソッドのpub(crate)化

actor-coreの内部実装メソッド36個が`pub(crate)`化されました。

#### 影響を受けるユーザー

actor-coreを直接使用している場合、以下のメソッドにアクセスできなくなります:
- SystemStateGeneric: register_cell, remove_cell, cell, ...
- DispatcherGeneric: register_invoker, enqueue_user, ...
- MailboxGeneric: enqueue_system, enqueue_user, ...

#### 移行方法

**推奨**: actor-std経由で使用

```rust
// 変更前（actor-core直接使用）
use cellactor_actor_core_rs::system::SystemStateGeneric;
let state = SystemStateGeneric::new();
state.register_cell(cell); // ❌ コンパイルエラー

// 変更後（actor-std使用）
use cellactor_actor_std_rs::system::ActorSystem;
let system = ActorSystem::new_empty(); // ✅ OK
```
```

## まとめ

この設計により、以下を実現:

1. **明確なAPI境界**: 公開API（12個）と内部実装（36個）の明確な分離
2. **保守性の向上**: 内部実装を自由に変更可能
3. **段階的移行**: Phase 1→2→3でリスクを管理
4. **ドキュメントの改善**: 公開APIが明確化
5. **セキュリティの向上**: 内部実装の隠蔽

破壊的変更ではあるが、actor-std経由での使用を推奨することで、多くのユーザーへの影響を最小化できる。
