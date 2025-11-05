# Capability: API可視性管理

## 概要

actor-coreパッケージにおいて、内部実装の詳細と公開APIを明確に区別し、適切な可視性制御を適用する。

## ADDED Requirements

### Requirement: SystemStateGeneric internal methods MUST be pub(crate)

SystemStateGeneric SHALL define all internal implementation methods (cell management, name management, child management, Future management) as `pub(crate)` and MUST NOT be accessible from outside the actor-core crate.

**ID**: REQ-API-VISIBILITY-001

**根拠**:
- これらのメソッドはactor-core内部のライフサイクル管理でのみ使用される
- 外部からの直接アクセスは不正な状態遷移を引き起こす可能性がある
- actor-std経由で安全なAPIを提供する

**影響を受けるコンポーネント**:
- `modules/actor-core/src/system/system_state.rs`
- SystemStateGeneric実装

#### Scenario: セル管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からregister_cell、remove_cell、cellメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn register_cell(&self, cell: ArcShared<ActorCellGeneric<TB>>) { ... }
    pub(crate) fn remove_cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }
    pub(crate) fn cell(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }
}
```

#### Scenario: 名前管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からassign_name、release_nameメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn assign_name(&self, parent: Option<Pid>, hint: Option<&str>, pid: Pid) -> Result<String, SpawnError> { ... }
    pub(crate) fn release_name(&self, parent: Option<Pid>, name: &str) { ... }
}
```

#### Scenario: 子管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からregister_child、unregister_child、child_pidsメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn register_child(&self, parent: Pid, child: Pid) { ... }
    pub(crate) fn unregister_child(&self, parent: &Pid, child: &Pid) { ... }
    pub(crate) fn child_pids(&self, parent: Pid) -> Vec<Pid> { ... }
}
```

#### Scenario: ガーディアン管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からset_user_guardian、clear_guardian、user_guardianメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn set_user_guardian(&self, pid: Pid) { ... }
    pub(crate) fn clear_guardian(&self, pid: Pid) -> bool { ... }
    pub(crate) fn user_guardian(&self) -> Option<ArcShared<ActorCellGeneric<TB>>> { ... }
}
```

#### Scenario: システムメッセージ管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からsend_system_message、notify_failureメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn send_system_message(&self, pid: Pid, msg: SystemMessage) -> Result<(), SendError<TB>> { ... }
    pub(crate) fn notify_failure(&self, pid: Pid, error: ActorError) { ... }
}
```

#### Scenario: Future管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からregister_ask_future、drain_ready_ask_futuresメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn register_ask_future(&self, future: ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>) { ... }
    pub(crate) fn drain_ready_ask_futures(&self) -> Vec<ArcShared<ActorFuture<AnyMessageGeneric<TB>, TB>>> { ... }
}
```

#### Scenario: エラー記録と終了管理メソッドはクレート内部でのみアクセス可能

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からrecord_send_error、mark_terminated、termination_futureメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub(crate) fn record_send_error(&self, sender: Option<Pid>, error: &SendError<TB>) { ... }
    pub(crate) fn mark_terminated(&self) { ... }
    pub(crate) fn termination_future(&self) -> ActorFuture<(), TB> { ... }
}
```

### Requirement: DispatcherGeneric internal methods MUST be pub(crate)

DispatcherGeneric SHALL define all internal implementation methods (message queuing, scheduling, Waker generation) as `pub(crate)` and MUST NOT be accessible from outside the actor-core crate.

**ID**: REQ-API-VISIBILITY-002

**根拠**:
- これらのメソッドはディスパッチャーの内部実装の詳細
- 外部からの直接操作はメッセージ順序の不整合を引き起こす可能性がある

**影響を受けるコンポーネント**:
- `modules/actor-core/src/dispatcher/base.rs`
- DispatcherGeneric実装

#### Scenario: メッセージキューイングメソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からenqueue_user、enqueue_systemメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn enqueue_user(&self, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> { ... }
    pub(crate) fn enqueue_system(&self, message: SystemMessage) { ... }
}
```

#### Scenario: スケジューリングメソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からscheduleメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn schedule(&self) { ... }
}
```

#### Scenario: インボーカー登録メソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からregister_invokerメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn register_invoker(&self, invoker: ArcShared<dyn MessageInvoker<TB>>) { ... }
}
```

#### Scenario: メールボックスアクセスメソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からmailboxメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn mailbox(&self) -> ArcShared<MailboxGeneric<TB>> { ... }
}
```

#### Scenario: Waker生成メソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からcreate_wakerメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn create_waker(&self) -> Waker { ... }
}
```

#### Scenario: Sender変換メソッドはクレート内部でのみアクセス可能

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からinto_senderメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub(crate) fn into_sender(self: ArcShared<Self>) -> ArcShared<dyn ActorRefSender<TB>> { ... }
}
```

### Requirement: MailboxGeneric internal methods MUST be pub(crate)

MailboxGeneric SHALL define all internal implementation methods (message queuing, dequeue, suspend/resume) as `pub(crate)` and MUST NOT be accessible from outside the actor-core crate.

**ID**: REQ-API-VISIBILITY-003

**根拠**:
- これらのメソッドはメールボックスの内部実装の詳細
- 外部からの直接操作はキュー状態の不整合を引き起こす可能性がある

**影響を受けるコンポーネント**:
- `modules/actor-core/src/mailbox/base.rs`
- MailboxGeneric実装

#### Scenario: キューイングメソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からenqueue_user、enqueue_systemメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn enqueue_user(&self, msg: AnyMessageGeneric<TB>) -> EnqueueOutcome { ... }
    pub(crate) fn enqueue_system(&self, msg: SystemMessage) { ... }
}
```

#### Scenario: 非同期キューイングメソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からenqueue_user_future、poll_user_futureメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn enqueue_user_future(&self, msg: AnyMessageGeneric<TB>) -> MailboxOfferFuture<TB> { ... }
    pub(crate) fn poll_user_future(&self, cx: &mut Context<'_>) -> Poll<MailboxPollFuture<TB>> { ... }
}
```

#### Scenario: デキューメソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からdequeueメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn dequeue(&self) -> Option<MailboxMessage<TB>> { ... }
}
```

#### Scenario: 一時停止/再開メソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からsuspend、resumeメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn suspend(&self) { ... }
    pub(crate) fn resume(&self) { ... }
}
```

#### Scenario: インストルメンテーション設定メソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からset_instrumentationメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn set_instrumentation(&self, instr: ArcShared<dyn MailboxInstrumentation<TB>>) { ... }
}
```

#### Scenario: 状態確認メソッドはクレート内部でのみアクセス可能

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からis_suspended、user_len、system_lenメソッドにアクセスしようとする
**THEN**: コンパイルエラーが発生する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub(crate) fn is_suspended(&self) -> bool { ... }
    pub(crate) fn user_len(&self) -> usize { ... }
    pub(crate) fn system_len(&self) -> usize { ... }
}
```

### Requirement: Public API methods MUST remain pub

Public API methods (constructors, test helpers, event stream access) SHALL remain `pub` and MUST be accessible from outside the actor-core crate.

**ID**: REQ-API-VISIBILITY-004

**根拠**:
- これらのメソッドは正当なユーザーAPIとして提供される
- actor-stdやexamplesから使用される

**影響を受けるコンポーネント**:
- `modules/actor-core/src/system/system_state.rs`
- `modules/actor-core/src/dispatcher/base.rs`
- `modules/actor-core/src/mailbox/base.rs`

#### Scenario: SystemStateGenericのコンストラクタは公開される

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からnewメソッドにアクセスする
**THEN**: アクセスが成功する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub fn new() -> Self { ... }
}
```

#### Scenario: SystemStateGenericのユーティリティメソッドは公開される

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からallocate_pid、monotonic_nowメソッドにアクセスする
**THEN**: アクセスが成功する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub fn allocate_pid(&self) -> Pid { ... }
    pub fn monotonic_now(&self) -> Duration { ... }
}
```

#### Scenario: SystemStateGenericのイベント/ログメソッドは公開される

**GIVEN**: SystemStateGenericが定義されている
**WHEN**: actor-core外部からevent_stream、publish_event、emit_logメソッドにアクセスする
**THEN**: アクセスが成功する

**実装**:
```rust
impl<TB: RuntimeToolbox> SystemStateGeneric<TB> {
    pub fn event_stream(&self) -> ArcShared<EventStreamGeneric<TB>> { ... }
    pub fn publish_event(&self, event: &EventStreamEvent<TB>) { ... }
    pub fn emit_log(&self, level: LogLevel, message: impl Into<String>, origin: Option<Pid>) { ... }
}
```

#### Scenario: DispatcherGenericのコンストラクタは公開される

**GIVEN**: DispatcherGenericが定義されている
**WHEN**: actor-core外部からnew、with_inline_executorメソッドにアクセスする
**THEN**: アクセスが成功する

**実装**:
```rust
impl<TB: RuntimeToolbox> DispatcherGeneric<TB> {
    pub fn new(mailbox: ArcShared<MailboxGeneric<TB>>, executor: ArcShared<dyn DispatchExecutor<TB>>) -> Self { ... }
    pub fn with_inline_executor(mailbox: ArcShared<MailboxGeneric<TB>>) -> Self { ... }
}
```

#### Scenario: MailboxGenericのコンストラクタは公開される

**GIVEN**: MailboxGenericが定義されている
**WHEN**: actor-core外部からnewメソッドにアクセスする
**THEN**: アクセスが成功する

**実装**:
```rust
impl<TB: RuntimeToolbox> MailboxGeneric<TB> {
    pub fn new(policy: MailboxPolicy) -> Self { ... }
}
```

## 検証基準

1. **コンパイル時検証**:
   - actor-core外部から`pub(crate)`メソッドへのアクセスがコンパイルエラーとなる
   - actor-core内部から`pub(crate)`メソッドへのアクセスが成功する
   - `pub`メソッドへのアクセスが内外から成功する

2. **機能テスト**:
   - 全既存テストがパスする
   - actor-stdのテストがパスする
   - examplesが正常に動作する

3. **ドキュメント検証**:
   - `cargo doc`で生成されるドキュメントに`pub(crate)`メソッドが含まれない
   - `pub`メソッドのドキュメントが適切に生成される

4. **CI/CD検証**:
   - `./scripts/ci-check.sh all`が成功する
   - 全lintがパスする
